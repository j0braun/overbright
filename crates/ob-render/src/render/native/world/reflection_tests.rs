use super::*;

#[test]
fn road_capture_detail_policy_preserves_native_and_follows_main_when_enabled() {
    assert_eq!(reflection_capture_quality(false, MAIN_LOD, 17.0), (LOW_LOD, 32.0));
    assert_eq!(reflection_capture_quality(false, MAIN_LOD_B, 17.0 / 3.0), (LOW_LOD, 32.0));
    assert_eq!(reflection_capture_quality(true, MAIN_LOD, 17.0), (MAIN_LOD, 17.0));
    assert_eq!(reflection_capture_quality(true, MAIN_LOD_B, 17.0 / 3.0), (MAIN_LOD_B, 17.0 / 3.0));
}

/// Synthetic receiving plane and high-frequency capture: no game assets.
/// Also validates array/multiview source variants and reflective fade/coverage pipelines.
#[test]
#[ignore = "requires a GPU adapter; run serially with --ignored"]
fn road_reflection_gpu_native_restore_angle_and_mask() {
    let gpu = pollster::block_on(Gpu::new(None, wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle()))).unwrap();
    let shared = WorldShared::new(None, gpu.build_tag());
    for arrays in [false, true] {
        if arrays && !gpu.device.features().contains(wgpu::Features::IMMEDIATES) { continue; }
        for multiview in [false, true] {
            if multiview && !gpu.device.features().contains(wgpu::Features::MULTIVIEW) { continue; }
            shared.mode(&gpu.device, arrays, multiview);
        }
        let mode = shared.mode(&gpu.device, arrays, false);
        for fade in [0, 1, 2] {
            for coverage in [false, true] {
                let key = PipelineKey { effect: Effect::Reflect, blend: Blend::None, alpha_test: true };
                make_pipeline(&gpu.device, &mode.layout, &mode.shader, key, wgpu::TextureFormat::Rgba8Unorm, wgpu::ColorWrites::ALL, None,
                    SceneMode { samples: if coverage { 4 } else { 1 }, reversed: false, coverage, hdr: false, lamps: false, modern: 0, bias_q: 0, lamp_features: 0 }, None, fade);
            }
        }
    }
    let mode = shared.mode(&gpu.device, false, false);
    let key = PipelineKey { effect: Effect::Reflect, blend: Blend::None, alpha_test: false };
    let pipeline = make_pipeline(&gpu.device, &mode.layout, &mode.shader, key, wgpu::TextureFormat::Rgba8Unorm, wgpu::ColorWrites::ALL, None, SceneMode::NATIVE, None, 0);
    let sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor { mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, ..Default::default() });
    let mut capture = Vec::new();
    for y in 0..64 { for x in 0..64 { let c = if (x + y) % 2 == 0 { 128 } else { 0 }; capture.extend([c, c, c, 255]); } }
    let capture_view = upload_rgba(&gpu, 64, 64, &capture);
    let reflection = bind_texture(&gpu.device, &make_texture_layout(&gpu.device, "reflection", false), &capture_view, &sampler);
    let vertices: Vec<Vertex> = [[-1.0, -1.0, 0.5], [1.0, -1.0, 0.5], [1.0, 1.0, 0.5], [-1.0, -1.0, 0.5], [1.0, 1.0, 0.5], [-1.0, 1.0, 0.5]].into_iter().map(|position| Vertex { position, color: [255; 4], uv: [0.5; 2] }).collect();
    let vertices = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(&vertices), usage: wgpu::BufferUsages::VERTEX });
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor { label: None, size: wgpu::Extent3d { width: 64, height: 64, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba8Unorm, usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC, view_formats: &[] });
    let color = target.create_view(&Default::default());
    let depth = create_depth(&gpu, 64, 64);
    let lamp_placeholder = super::lamps::LampBindings::placeholder(&gpu.device);
    let render = |enhanced: bool, eye: Vec3, mask: u8, surface: Mat4| {
        let instances = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(&surface.to_cols_array()), usage: wgpu::BufferUsages::VERTEX });
        let material_view = upload_rgba(&gpu, 1, 1, &[16, 16, 16, mask]);
        let material = bind_world_texture(&gpu.device, &make_texture_layout(&gpu.device, "material", true), &sampler, &material_view, &Scroll::default());
        let uniform = FrameUniform { view_proj: surface.inverse().to_cols_array_2d(), camera: surface.transform_point3(eye).extend(1.0).to_array(), forward: [0.0, 0.0, 1.0, 0.0], fog_value: [0.0, 0.0, 1.0, 0.0], fog_color: [0.0; 4], clip: [if enhanced { 1.0 } else { f32::MIN }, 0.0, 0.0, 0.0], cull: [0.0; 4], headlight: [[0.0; 4]; 5], wet: [0.0; 4], lamps: [0.0; 4], cluster: [0.0; 4], cluster_near: [0.0; 4] };
        let buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::bytes_of(&uniform), usage: wgpu::BufferUsages::UNIFORM });
        let frame = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &frame_layout(&gpu.device), entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&material_view) },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&sampler) },
        ].into_iter().chain(lamp_placeholder.entries()).collect::<Vec<_>>() });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor { label: None, color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &color, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } })], depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &depth, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }), stencil_ops: None }), timestamp_writes: None, occlusion_query_set: None, multiview_mask: None });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &frame, &[]);
            pass.set_bind_group(1, &material, &[]);
            pass.set_bind_group(2, &reflection, &[]);
            pass.set_vertex_buffer(0, vertices.slice(..));
            pass.set_vertex_buffer(1, instances.slice(..));
            pass.draw(0..6, 0..1);
        }
        gpu.queue.submit([encoder.finish()]);
        read_back(&gpu, &target, 64, 64)
    };
    let eye = Vec3::new(0.0, 0.0, 3.0);
    let native = render(false, eye, 255, Mat4::IDENTITY);
    for (pixel, source) in native.chunks_exact(4).zip(capture.chunks_exact(4)) {
        assert_eq!(pixel, &[source[0] + 32, source[0] + 32, source[0] + 32, 255], "native formula remains 2*base + reflection");
    }
    let enhanced = render(true, eye, 255, Mat4::IDENTITY);
    assert!(native != enhanced, "enhanced mode must affect reflected pixels");
    assert!(native == render(false, eye, 255, Mat4::IDENTITY), "switching off restores native bytes");
    let tilted = render(true, eye, 255, Mat4::from_rotation_x(0.7));
    assert!(enhanced.iter().zip(&tilted).all(|(&a, &b)| a.abs_diff(b) <= 1), "rotating surface and camera together preserves Fresnel response");
    let grazing = render(true, Vec3::new(0.0, -100.0, 3.0), 255, Mat4::IDENTITY);
    let energy = |image: &[u8]| image.chunks_exact(4).map(|p| u64::from(p[0])).sum::<u64>();
    assert!(energy(&grazing) > energy(&enhanced), "grazing wet surfaces reflect more");
    assert!(render(false, eye, 0, Mat4::IDENTITY) == render(true, eye, 0, Mat4::IDENTITY), "zero authored mask leaves base untouched");
    let contrast = |image: &[u8]| image.chunks_exact(4).map(|p| p[0]).max().unwrap() - image.chunks_exact(4).map(|p| p[0]).min().unwrap();
    assert!(contrast(&enhanced) < contrast(&native), "filter reduces checkerboard aliasing");
}


#[test]
#[ignore = "requires a GPU adapter; run serially with --ignored"]
fn environment_capture_enhanced_detail_and_native_restore() {
    let _globals = crate::qol::test_globals();
    let gpu = pollster::block_on(Gpu::new(None, wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle()))).unwrap();
    let mut world = World::default();
    world.solids.push(crate::formats::solid::Solid {
        name: "capture fixture".into(), hash: 1,
        bbox_min: [-1.0, -1.0, -1.0], bbox_max: [1.0, 1.0, 1.0],
        textures: vec![], normals: vec![], light_materials: vec![],
        vertices: [[0.0, -1.0, -1.0], [0.0, 1.0, -1.0], [0.0, 0.0, 1.0]].into_iter()
            .map(|position| Vertex { position, color: [255; 4], uv: [0.0; 2] }).collect(),
        indices: vec![0, 1, 2, 2, 1, 0],
        materials: vec![crate::formats::solid::Material { texture_slot: 0, light_material: 255, first_index: 0, index_count: 6, flags: 0 }],
        mesh_flags: 0, markers: vec![], light_markers: vec![],
    });
    world.placements.push(crate::world::Placement {
        solid: 0, mid_solid: Some(0), low_solid: None,
        transform: Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)),
        section: 1, flags: 0x60, cull_radius: 1.0,
    });
    // A second, native-visible prop makes restoration a non-black image check.
    world.placements.push(crate::world::Placement {
        solid: 0, mid_solid: Some(0), low_solid: Some(0),
        transform: Mat4::from_translation(Vec3::new(10.0, 4.0, 0.0)),
        section: 1, flags: 0x60, cull_radius: 2.0,
    });
    let mut renderer = WorldRenderer::new(&gpu, &world, wgpu::TextureFormat::Rgba8Unorm, &crate::render::RenderOptions::in_force());
    // A readable six-layer capture replaces only this fixture's render targets.
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("capture regression"), size: wgpu::Extent3d { width: ENV_SIZE, height: ENV_SIZE, depth_or_array_layers: 6 },
        mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
        format: REFLECTION_FORMAT, usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC, view_formats: &[],
    });
    let core = std::sync::Arc::get_mut(&mut renderer.core).unwrap();
    core.env.faces = (0..6).map(|i| target.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2), base_array_layer: i, array_layer_count: Some(1), ..Default::default()
    })).collect();
    if let Some(m) = &mut core.env_multiview {
        m.color = target.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2Array), ..Default::default() });
    }
    let camera = Camera::looking_at(Vec3::ZERO, Vec3::Y);
    let params = FrameParams { camera: &camera, aspect: 1.0, visible_sections: None, far_sections: None,
        fog: None, time: 0.0, road_reflections: false, road_reflection_plane: None, sky: SkyFrame::default(), reversed: false,
        headlight_pools: &[], reflection_glows: &[], display: display::Display::ALL, opts: &crate::render::RenderOptions::in_force(), rear_view: None, occluders: &[] };
    let mut frames = Vec::new();
    for enhanced in [false, true, false] {
        crate::qol::Enhancements { enhanced_reflections: enhanced, ..Default::default() }.apply();
        let plan = renderer.plan_environment(&gpu, Vec3::ZERO, (Vec3::X, -Vec3::Z, Vec3::Y), &FrameParams { opts: &crate::render::RenderOptions::in_force(), ..params }, [true; 6], true);
        assert_eq!(plan.lod, if enhanced { MAIN_LOD } else { LOW_LOD });
        let expected_size = if enhanced { MAIN_MIN_SIZE } else { LOW_LOD_MIN_SIZE };
        assert!(plan.faces.iter().all(|(_, face)| face.min_size == expected_size), "enhanced face culling follows main-view detail");
        // A UI toggle after planning must not change asynchronous recording.
        crate::qol::Enhancements::default().apply();
        let groups = renderer.core.kept_groups(&plan.visible, &plan.cube, WorldView::Environment, false, false, None, plan.lod);
        assert_eq!(groups.len(), 1 + usize::from(enhanced), "main-only prop is present only in enhanced capture");
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        renderer.core.record_environment(&mut encoder, &plan);
        gpu.queue.submit([encoder.finish()]);
        frames.push(read_back(&gpu, &target, ENV_SIZE, ENV_SIZE));
    }
    assert_eq!(frames[0], frames[2], "native capture restored byte-for-byte");
    let visible_pixels = |frame: &[u8]| frame.chunks_exact(4).filter(|p| p[..3] != [0; 3]).count();
    assert!(visible_pixels(&frames[0]) > 20, "native-visible prop must render");
    assert!(visible_pixels(&frames[1]) > visible_pixels(&frames[0]) + 20, "enhanced main-only prop must reach captured pixels");
    println!("capture pixels native={} enhanced={}, multiview={}", visible_pixels(&frames[0]), visible_pixels(&frames[1]), renderer.core.env_multiview.is_some());
    if let Some(directory) = crate::render::debug_env::nfsu2_capture_fixture_dir() {
        for (name, pixels) in ["native", "enhanced", "restored"].into_iter().zip(&frames) {
            image::save_buffer(std::path::Path::new(&directory).join(format!("{name}.png")), pixels, ENV_SIZE, ENV_SIZE, image::ColorType::Rgba8).unwrap();
        }
    }
    // Camera rotation must not change the coordinate frame of retained faces.
    crate::qol::Enhancements { enhanced_reflections: true, ..Default::default() }.apply();
    let capture = |renderer: &mut WorldRenderer, centre, axes, faces| {
        let plan = renderer.plan_environment(&gpu, centre, axes, &FrameParams { opts: &crate::render::RenderOptions::in_force(), ..params }, faces, true);
        let count = plan.faces.len();
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        renderer.core.record_environment(&mut encoder, &plan);
        gpu.queue.submit([encoder.finish()]);
        (count, read_back(&gpu, &target, ENV_SIZE, ENV_SIZE))
    };
    let axes = (Vec3::X, -Vec3::Z, Vec3::Y);
    let (count, before) = capture(&mut renderer, Vec3::ZERO, axes, [true, false, false, false, false, false]);
    assert_eq!(count, 6, "basis transition refreshes all retained faces");
    let turn = glam::Quat::from_rotation_z(1.1) * glam::Quat::from_rotation_x(0.4);
    let rotated = (turn * axes.0, turn * axes.1, turn * axes.2);
    let (count, after) = capture(&mut renderer, Vec3::ZERO, rotated, [true, false, false, false, false, false]);
    assert_eq!(count, 1, "ordinary enhanced frames preserve the selected face budget");
    assert_eq!(before, after, "camera yaw/pitch cannot rotate a partial face away from older faces");
    let (_, moved) = capture(&mut renderer, Vec3::new(0.0, 0.75, 0.0), rotated, [true, false, false, false, false, false]);
    let (_, moved_full) = capture(&mut renderer, Vec3::new(0.0, 0.75, 0.0), axes, [true; 6]);
    assert_eq!(moved, moved_full, "updated face at a translated capture centre matches a full capture independently of camera axes");
    crate::qol::Enhancements::default().apply();
    let (count, restored) = capture(&mut renderer, Vec3::ZERO, axes, [true, false, false, false, false, false]);
    assert_eq!(count, 6, "return to native replaces world-oriented faces");
    assert_eq!(restored, frames[0], "partial update on toggle restores native image exactly");
    for lod_b in [false, true] {
        let mut params = FrameParams { display: display::Display { world_lod_b: lod_b, ..display::Display::ALL }, ..params };
        crate::qol::Enhancements { enhanced_reflections: true, detail_distance: 300, ..Default::default() }.apply();
        let plan = renderer.plan_environment(&gpu, Vec3::ZERO, (Vec3::X, -Vec3::Z, Vec3::Y), &FrameParams { opts: &crate::render::RenderOptions::in_force(), ..params }, [true; 6], true);
        assert_eq!(plan.lod, if lod_b { MAIN_LOD_B } else { MAIN_LOD });
        assert!(plan.faces.iter().all(|(_, face)| face.min_size == MAIN_MIN_SIZE / 3.0));
        params.display.env_world = false;
        let plan = renderer.plan_environment(&gpu, Vec3::ZERO, (Vec3::X, -Vec3::Z, Vec3::Y), &FrameParams { opts: &crate::render::RenderOptions::in_force(), ..params }, [true; 6], false);
        assert!(plan.faces.is_empty());
        assert_eq!(plan.cleared, (0..6).collect::<Vec<_>>());
    }
    crate::qol::Enhancements { enhanced_reflections: true, car_reflection: 256, ..Default::default() }.apply();
    assert!(renderer.sync_environment_size(&gpu, &crate::render::RenderOptions::in_force()));
    let plan = renderer.plan_environment(&gpu, Vec3::ZERO, axes, &FrameParams { opts: &crate::render::RenderOptions::in_force(), ..params }, [true, false, false, false, false, false], true);
    assert_eq!(plan.faces.len(), 6, "new enhanced cube storage initializes every face");
    let plan = renderer.plan_environment(&gpu, Vec3::ZERO, axes, &FrameParams { opts: &crate::render::RenderOptions::in_force(), ..params }, [true, false, false, false, false, false], true);
    assert_eq!(plan.faces.len(), 1, "size transition does not permanently override the update budget");
    crate::qol::Enhancements::default().apply();
}

#[test]
#[ignore = "requires original assets and a GPU; run serially"]
fn car_dynamic_cube_lookup_and_static_frontend_convention() {
    let _globals = crate::qol::test_globals();
    let gpu = pollster::block_on(Gpu::new(None, Gpu::new_instance())).unwrap();
    let car = crate::car::Car::load(&crate::game_root(), "RX8").unwrap();
    let mut renderer = car::CarRenderer::new(&gpu, &car, wgpu::TextureFormat::Rgba8Unorm, &crate::render::RenderOptions::in_force());
    let cube = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("asymmetric cube fixture"), size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 6 },
        mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm, usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[],
    });
    let colors: [[u8; 4]; 6] = [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255], [255, 255, 0, 255], [255, 0, 255, 255], [0, 255, 255, 255]];
    gpu.queue.write_texture(cube.as_image_copy(), bytemuck::cast_slice(&colors), wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4), rows_per_image: Some(1) }, cube.size());
    let cube = cube.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::Cube), ..Default::default() });
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: None, size: wgpu::Extent3d { width: 320, height: 240, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1,
        dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC, view_formats: &[],
    });
    let color = target.create_view(&Default::default());
    let depth = create_depth(&gpu, 320, 240);
    let camera = Camera::looking_at(Vec3::new(4.0, -6.0, 2.5), Vec3::new(0.0, 0.0, 0.5));
    let lighting = lighting::CarLighting::frontend(&lighting::DEFAULT_RIG, &camera, Mat4::IDENTITY);
    let render = |renderer: &mut car::CarRenderer, enhanced, dynamic, view| {
        crate::qol::Enhancements { enhanced_reflections: enhanced, ..Default::default() }.apply();
        if dynamic { renderer.set_world_environment(&gpu, &cube); } else { renderer.set_environment(&gpu, &cube); }
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        {
            let _clear = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None, color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &color, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &depth, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }), stencil_ops: None }), timestamp_writes: None, occlusion_query_set: None, multiview_mask: None,
            });
        }
        renderer.render(&gpu, &mut encoder, &color, &depth, camera.view_projection(4.0/3.0), view, &lighting, Mat4::IDENTITY, &[crate::car::WheelPose::default(); 4], &crate::render::RenderOptions::in_force());
        gpu.queue.submit([encoder.finish()]);
        read_back(&gpu, &target, 320, 240)
    };
    let view = camera.view();
    let rotated = Mat4::from_rotation_y(1.2) * view;
    let native = render(&mut renderer, false, false, view);
    assert_eq!(native, render(&mut renderer, false, true, view), "native dynamic binding keeps original lookup");
    let enhanced = render(&mut renderer, true, true, view);
    assert_eq!(enhanced, render(&mut renderer, true, true, rotated), "dynamic enhanced lookup is independent of camera basis supplied to cube lookup");
    let static_image = render(&mut renderer, true, false, view);
    assert_ne!(static_image, render(&mut renderer, true, false, rotated), "static frontend cubes retain camera-relative lookup even when rebinding same texture");
    assert_eq!(native, render(&mut renderer, false, true, view), "native restore is byte exact");
    assert!(enhanced.chunks_exact(4).filter(|p| p[..3] != [0; 3]).count() > 1000);
    if let Some(directory) = crate::render::debug_env::nfsu2_capture_fixture_dir() {
        for (name, pixels) in [("car-native", native), ("car-world", enhanced), ("car-static", static_image)] {
            image::save_buffer(std::path::Path::new(&directory).join(format!("{name}.png")), &pixels, 320, 240, image::ColorType::Rgba8).unwrap();
        }
    }
    crate::qol::Enhancements::default().apply();
}

#[test]
fn local_road_plane_transform_preserves_receiver_and_mirrors_height() {
    for height in [-12.0, 0.0, 5.0, 100.0] {
        let mirror = road_plane_matrix(height);
        let receiver = Vec3::new(3.0, -7.0, height);
        assert_eq!(mirror.transform_point3(receiver), receiver);
        assert_eq!(mirror.transform_point3(receiver + Vec3::Z * 3.0), receiver - Vec3::Z * 3.0);
        assert_eq!(mirror * mirror, Mat4::IDENTITY);
    }
}

#[test]
#[ignore = "requires a GPU; run serially"]
fn road_planar_capture_geometry_effects_and_native_fallback() {
    let _globals = crate::qol::test_globals();
    let gpu = pollster::block_on(Gpu::new(None, Gpu::new_instance())).unwrap();
    let camera = Camera::looking_at(Vec3::new(0.0, -5.0, 8.0), Vec3::new(0.0, 8.0, 5.0));
    // Exact 90-degree rotation mixes local Y/Z: negating only m22 cannot
    // reflect this placement, while baked null-transform geometry must agree.
    let transform = Mat4::from_cols(Vec3::X.extend(0.0), Vec3::Z.extend(0.0), (-Vec3::Y).extend(0.0), Vec3::new(0.0, 8.0, 5.0).extend(1.0));
    let local = [Vec3::new(-1.0, 1.0, 0.0), Vec3::new(1.0, 1.0, 0.0), Vec3::new(0.0, 3.0, 0.0)];
    let make = |transformed, offset: f32| {
        let vertices: Vec<Vertex> = local.into_iter().map(|p| Vertex {
            position: if transformed { p.to_array() } else { (transform.transform_point3(p) + Vec3::Z * offset).to_array() }, color: [255; 4], uv: [0.0; 2],
        }).collect();
        let min = vertices.iter().fold(Vec3::splat(f32::INFINITY), |v, p| v.min(Vec3::from(p.position)));
        let max = vertices.iter().fold(Vec3::splat(f32::NEG_INFINITY), |v, p| v.max(Vec3::from(p.position)));
        let mut world = World::default();
        world.solids.push(crate::formats::solid::Solid {
            name: "planar capture fixture".into(), hash: 1, bbox_min: min.to_array(), bbox_max: max.to_array(),
            textures: vec![], normals: vec![], light_materials: vec![], vertices, indices: vec![0, 1, 2, 2, 1, 0],
            materials: vec![crate::formats::solid::Material { texture_slot: 0, light_material: 255, first_index: 0, index_count: 6, flags: MESH_FLAG_REFLECTIVE }],
            mesh_flags: MESH_FLAG_REFLECTIVE, markers: vec![], light_markers: vec![],
        });
        world.placements.push(crate::world::Placement {
            solid: 0, mid_solid: Some(0), low_solid: Some(0), transform: if transformed { transform } else { Mat4::IDENTITY },
            section: 1, flags: 0x60 | if transformed { 0 } else { NO_MATRIX_FLAG }, cull_radius: 0.0,
        });
        WorldRenderer::new(&gpu, &world, wgpu::TextureFormat::Rgba8Unorm, &crate::render::RenderOptions::in_force())
    };
    let pools = [HeadlightPool { transform: Mat4::from_translation(Vec3::new(-13.0, 8.0, 3.0)), color: [80, 100, 120] }];
    let glows = [crate::particles::Quad { texture: 0, corners: [Vec3::new(-1.0, 8.0, 1.0), Vec3::new(1.0, 8.0, 1.0), Vec3::new(1.0, 8.0, 3.0), Vec3::new(-1.0, 8.0, 3.0)], uv: [[0.0; 2]; 4], rgba: [80, 100, 120, 255] }];
    let render = |renderer: &mut WorldRenderer, enhanced, height, effects| {
        crate::qol::Enhancements { enhanced_reflections: enhanced, ..Default::default() }.apply();
        let params = FrameParams { camera: &camera, aspect: 4.0/3.0, visible_sections: None, far_sections: None, fog: None, time: 0.0,
            road_reflections: true, road_reflection_plane: height, sky: SkyFrame::default(), reversed: false,
            headlight_pools: if effects == 1 { &pools } else { &[] }, reflection_glows: if effects == 2 { &glows } else { &[] }, display: display::Display::ALL, opts: &crate::render::RenderOptions::in_force(), rear_view: None, occluders: &[] };
        let plan = renderer.plan_views(&gpu, &params);
        let (cull, color, depth) = plan.reflection.as_ref().expect("reflective fixture requests capture");
        if enhanced && height.is_some_and(f32::is_finite) {
            assert!(!cull.mirrored, "planar culling uses original geometry bounds");
            assert_eq!(cull.eye.z, 2.0 * height.unwrap() - camera.position.z);
            assert_eq!(cull.forward.z, -camera.forward().z);
        }
        let order = renderer.group_order(&renderer.kept_groups(&plan.visible, cull, WorldView::RoadReflection, false, false, None, plan.reflection_lod));
        if effects != 0 { assert!(order.is_empty(), "effect fixture excludes scenery"); }
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        let view = if cull.mirrored { WorldView::RoadReflection } else { WorldView::PlanarRoadReflection };
        renderer.draw_pass(&mut encoder, color, depth, wgpu::Color::BLACK, &renderer.reflection_frame.1, &renderer.no_reflection, &order, 0.0, None, view, (plan.pool_count, plan.reflection_glow_count));
        gpu.queue.submit([encoder.finish()]);
        let (target, (w, h)) = renderer.reflection_image().unwrap();
        read_back(&gpu, target, w, h)
    };
    let mut transformed = make(true, 0.0);
    let mut baked = make(false, 0.0);
    let native = render(&mut baked, false, Some(5.0), 0);
    let aligned = render(&mut baked, true, Some(5.0), 0);
    assert_eq!(aligned, render(&mut transformed, true, Some(5.0), 0), "transformed and null-transform copies of identical world geometry align");
    assert_ne!(native, aligned, "plane capture corrects native unmirrored null-transform geometry");
    assert_eq!(native, render(&mut baked, false, Some(5.0), 0), "native restoration is byte exact");
    assert_eq!(native, render(&mut baked, true, None, 0), "frontend/missing-ground fallback keeps native capture geometry");
    assert_eq!(native, render(&mut baked, true, Some(f32::NAN), 0), "invalid ground height cannot contaminate capture matrix");
    let lit = |image: &[u8]| image.chunks_exact(4).filter(|p| p[..3] != [0; 3]).count();
    assert!(lit(&aligned) > 20, "aligned scenery must reach the capture");
    let expected = camera.view_projection(4.0/3.0).project_point3(road_plane_matrix(5.0).transform_point3(Vec3::new(0.0, 8.0, 20.0/3.0)));
    let expected_y = (0.5 - expected.y * 0.5) * 240.0;
    let mean_y = aligned.chunks_exact(4).enumerate().filter(|(_, p)| p[..3] != [0; 3]).map(|(i, _)| (i / 320) as f32).sum::<f32>() / lit(&aligned) as f32;
    assert!((mean_y - expected_y).abs() < 4.0, "reflection lies at geometrically projected road-plane location: {mean_y} vs {expected_y}");
    let mut below = make(false, -4.0);
    assert!(lit(&render(&mut below, false, Some(5.0), 0)) > 20);
    assert_eq!(lit(&render(&mut below, true, Some(5.0), 0)), 0, "below-plane geometry cannot occlude reflected scenery");
    // Isolate the actual effect draw paths: no scenery, existing texture slot,
    // below-plane glow and pool coordinates that would be clipped/double-mirrored.
    let core = std::sync::Arc::get_mut(&mut baked.core).unwrap();
    core.section_groups.iter_mut().for_each(Vec::clear);
    core.pool_texture = Some((0, PipelineKey { effect: Effect::NoFog, blend: Blend::Additive, alpha_test: false }));
    core.reflection_glow_texture = core.pool_texture;
    for effects in [1, 2] {
        let original = render(&mut baked, false, Some(5.0), effects);
        assert!(lit(&original) > 20, "effect fixture must be visible ({effects})");
        assert!(original == render(&mut baked, true, Some(5.0), effects), "authored effect placement survives planar scenery capture ({effects})");
    }
    if let Some(directory) = crate::render::debug_env::nfsu2_capture_fixture_dir() {
        for (name, pixels) in [("road-native", native), ("road-planar", aligned)] {
            image::save_buffer(std::path::Path::new(&directory).join(format!("{name}.png")), &pixels, 320, 240, image::ColorType::Rgba8).unwrap();
        }
    }
    crate::qol::Enhancements::default().apply();
}

/// The GPS arrow must not be visible in the
/// reflections (docs/notes/graphics/reflections.md "The GPS arrow kept out of reflections"): MARKER_GPS_AID goes in with flags 0 to the views
/// (`World::add_gps_aid`: HIDDEN_FLAG only, which the views test per
/// instance), so the road reflection (planar or native), the cube and the
/// rear-view mirror cull it (their 0x40) and only the main view draws it,
/// as the game draws it in the player's view alone (0x631767).
#[test]
fn gps_aid_is_drawn_in_the_main_view_only() {
    let flags = crate::world::HIDDEN_FLAG & VIEW_FLAGS;
    assert_eq!(flags, 0);
    for reversed in [false, true] {
        assert!(!WorldView::Main.culls(flags, reversed));
        for view in [WorldView::RoadReflection, WorldView::PlanarRoadReflection, WorldView::Environment, WorldView::RearView] {
            assert!(view.culls(flags, reversed), "{view:?} reversed {reversed}");
        }
    }
}

/// The overlays' draws (the SSR mask's flag): every draw of the groups of
/// the placements' slots, ascending, once each; none for no placement.
#[test]
fn overlay_draws_are_the_placements_group_draws() {
    let placement_slots = vec![vec![0], vec![1, 2], vec![3]];
    let slot_group = vec![0, 1, 1, 2];
    let group_draws = vec![vec![0, 1], vec![4, 2], vec![3]];
    assert_eq!(overlay_draws([1], &placement_slots, &slot_group, &group_draws), vec![2, 4]);
    assert_eq!(overlay_draws([2, 0], &placement_slots, &slot_group, &group_draws), vec![0, 1, 3]);
    assert!(overlay_draws(None, &placement_slots, &slot_group, &group_draws).is_empty());
    assert!(overlay_draws([9], &placement_slots, &slot_group, &group_draws).is_empty());
}
