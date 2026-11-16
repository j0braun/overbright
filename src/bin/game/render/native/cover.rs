use crate::*;

/// UI_MagazineView's shot: the car, its renderer, the shot's target, the
/// picked layers' textures and the picked environment map's six faces.
pub(crate) struct CoverShot {
    pub(crate) car: Car,
    pub(crate) renderer: Option<CarRenderer>,
    pub(crate) target: Option<wgpu::Texture>,
    pub(crate) streamed: Vec<nfsu2::formats::texture::Texture>,
    pub(crate) envmap: Option<Vec<nfsu2::formats::texture::Texture>>,
    pub(crate) rig: [nfsu2::render::lighting::RigLight; 3],
    /// The shot read back (RGBA, top down, width, height) for the reward's
    /// save (`LotDeal::SaveCover`).
    pub(crate) pixels: Option<(Vec<u8>, u32, u32)>,
}

/// A movie's new frame to its texture.
#[cfg_attr(not(feature = "dev-harness"), allow(dead_code))] // the shots' (the window's frame splits it)
pub(crate) fn upload_movie(mode: &mut Mode, gpu: &Gpu, ui: &mut UiRenderer) {
    if upload_movie_frame(mode, gpu, ui) {
        movie_shown_update(mode);
    }
}

/// [`upload_movie`]'s upload (the draw's half): whether a fresh
/// frame went up.
pub(crate) fn upload_movie_frame(mode: &Mode, gpu: &Gpu, ui: &mut UiRenderer) -> bool {
    if let Mode::Menu(Menu::Movie(m)) = mode
        && let Some(p) = m.player.as_ref()
        && p.fresh
    {
        ui.set_rgba(gpu, MOVIE_TEXTURE, p.width() as u32, p.height() as u32, &p.rgba);
        return true;
    }
    false
}

/// [`upload_movie`]'s update (after the draw): the frame shown.
pub(crate) fn movie_shown_update(mode: &mut Mode) {
    if let Mode::Menu(Menu::Movie(m)) = mode
        && let Some(p) = m.player.as_mut()
    {
        p.fresh = false;
    }
}

/// Renders UI_MagazineView's shot (once per view) and binds it under
/// `CoverView::shot()`: the photo shoot's screen at 640 × 480 — the
/// background layer (UI_MagazineBack), the car at the origin turned by the
/// camera's +0x08 (degrees: 0x4C2070 sets the FE car object's turntable
/// angle, current +0x254 and target +0x274, which 0x4C1D40 wraps by 360;
// prov: inferred game.upload_movie
/// **inferred**: about +z) seen from the camera's eye towards its point
/// (0x444DE0) with the lens (+0x10) as the horizontal field of view in
/// degrees (0x44D116 writes it to the camera's +0xC4 as an angle, × 65536
// prov: inferred game.upload_movie-2
/// / 360; **inferred**: across the screen's width, h / w of 0x5C7ED0), then
/// the masthead, model and extra (UI_MagazineReward) — cut to `CoverView::CROP`
/// (0x5C3130).
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_cover(mode: &Mode, shot: &mut Option<CoverShot>, picture: &mut Option<Vec<u8>>, gpu: &Gpu, format: wgpu::TextureFormat, ui: &mut UiRenderer, assets: &Assets, settings: &nfsu2::render::FrameSettings) {
    let (display, opts) = (settings.display, &settings.opts);
    let Mode::Menu(Menu::Cover(view)) = mode else { return };
    // UI_MagazineView's loaded picture, as stored (the 512² capture texture).
    if view.picture {
        if let Some(rgba) = picture.take() {
            let n = nfsu2::formats::native_save::PICTURE_SIZE;
            ui.set_rgba(gpu, screens::CoverView::shot(), n, n, &rgba);
        }
        return;
    }
    let (Some(camera), Some(shot)) = (view.camera, shot.as_mut()) else { return };
    if shot.target.is_some() {
        return;
    }
    let make = |label, (width, height): (u32, u32), usage| {
        gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    };
    let (width, height) = screens::CoverView::SCREEN;
    let screen = make("photo shoot", (width, height), wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC);
    let target = screen.create_view(&Default::default());
    for t in &shot.streamed {
        ui.add_texture(gpu, t);
    }
    let (behind, over) = screens::CoverView::layer_quads(assets, view.layers);
    ui.render(gpu, assets, &target, (width, height), &behind, true);
    let depth = create_depth(gpu, width, height);
    let rig = &shot.rig;
    let renderer = shot.renderer.get_or_insert_with(|| CarRenderer::new(gpu, &shot.car, format, opts));
    if let Some(cube) = shot.envmap.as_deref().and_then(|faces| nfsu2::render::car::static_envmap(gpu, faces)) {
        renderer.set_environment(gpu, &cube);
    }
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    {
        let _clear = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("cover depth"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    }
    let lens = camera.lens.to_radians();
    let mut eye = Camera::look_at(camera.eye, camera.target, camera.up(), lens);
    // The lens is the FE camera's angle (+0xC4, 0x44D116), which view 1's
    // projection 0x5C7ED0 widens as for GarageMain's shots (×1.1 across,
    // `Camera::game_fov`); the composition is the 4:3 screen whatever the
    // window's shape. The clone took the lens unwidened (the car ~10% too
    // big).
    eye.game_fov = Some(lens);
    let view_projection = eye.view_projection(width as f32 / height as f32);
    // The car is the FE car of GarageMain (0x4A7830(0); UI_MagazineReward's
    // constructor 0x565760 finds GarageMain.fng, 0x565A2C), posed in place
    // on the platform's front disc: 0x4C2070 sets only its turntable angle
    // (+0x274 / +0x254) and steer, not the platform (0x8363F0) or its
    // disc, so it stands where GarageMain shows it — the front disc's
    // centre (0, 5.4), the tyres 0.15 up (0x7F45A8) — and the cameras' eye
    // and point are in that same world (eye ≈ (0, −4, 2.3), as GarageMain's
    // own shots, `garage::SHOTS`). The clone drew it at the origin with the
    // body's origin on the floor, 5.4 m too near and sunk.
    let turn = nfsu2::garage::Turntable { platform_angle: 0.0, disc: 0, car_angle: camera.car[0] }.car_transform(&shot.car.car_type.wheels);
    // +0x0C is the front wheels' steer in degrees: 0x4C2070 sets it as the
    // FE car's target (+0x25C) and current (0x7F45A4, 16 by default), and
    // the FE car's wheel matrices (0x4A6E8B) turn wheels 0 and 1 by it
    // about +z (0x5BAE70 → 0x5BACA0, deg × 65536 / 360).
    let steer = camera.car[1].to_radians();
    let wheels: [nfsu2::car::WheelPose; 4] = std::array::from_fn(|i| nfsu2::car::WheelPose { steer: if i < 2 { steer } else { 0.0 }, ..Default::default() });
    // No matching environment skips the native rig store. DEFAULT is
    // the existing approximation for that case; inherited FE state is open.
    // The native reward queue places SHADOWFE at (non-late, order 2,
    // effect 13), between car effect 5's order <=2 and later buckets.
    // Native capture: docs/evidence/cover-shadow, calls 60580460/485/510.
    let shadow = display.shadows.then(|| {
        let texture = shot.car.texture_for(nfsu2::formats::hash::bin_hash(&format!("{}_SHADOWFE", shot.car.name)))?;
        let grid = nfsu2::shadow::car_shadow(turn, shot.car.shadow_box, shot.car.car_type.wheels.map(|w| w.position), None)?
            .toward_eye(eye.position, turn.w_axis.truncate(), nfsu2::shadow::shadow_pull(None));
        Some((texture, grid))
    }).flatten();
    let mut shadow_renderer = shadow.as_ref().map(|(texture, _)| {
        let mut renderer = WorldRenderer::new(gpu, &World::default(), format, opts);
        renderer.add_shadow_texture(gpu, texture, opts);
        renderer
    });
    renderer.render_cover(gpu, &mut encoder, &target, &depth, view_projection, eye.view(), &CarLighting::frontend(rig, &eye, turn), turn, &wheels, |encoder| {
        if let (Some(renderer), Some((texture, grid))) = (&mut shadow_renderer, shadow) {
            renderer.render_cover_shadow(gpu, encoder, &target, &depth, view_projection, &[(texture.hash, grid)]);
        }
    }, opts);
    gpu.queue.submit([encoder.finish()]);
    ui.render(gpu, assets, &target, (width, height), &over, false);
    let ((x, y), size) = screens::CoverView::CROP;
    let texture = make("cover shot", size, wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING);
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_texture(
        wgpu::TexelCopyTextureInfo { texture: &screen, mip_level: 0, origin: wgpu::Origin3d { x, y, z: 0 }, aspect: wgpu::TextureAspect::All },
        wgpu::TexelCopyTextureInfo { texture: &texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
    );
    gpu.queue.submit([encoder.finish()]);
    ui.set_texture(gpu, screens::CoverView::shot(), &texture.create_view(&Default::default()));
    shot.target = Some(texture);
    // The shot on the CPU for the reward's save (the original keeps its
    // capture in the 512² "ScreenCap" texture, 0x832DAC).
    if view.reward {
        // As RGBA8 whatever the window's format; one it cannot convert
        // leaves the reward without a saved picture.
        match nfsu2::render::read_back_rgba8(gpu, &screen) {
            Ok(all) => {
                let mut rgba = Vec::with_capacity((size.0 * size.1 * 4) as usize);
                for row in y..y + size.1 {
                    let start = ((row * width + x) * 4) as usize;
                    for p in all[start..start + (size.0 * 4) as usize].chunks_exact(4) {
                        rgba.extend_from_slice(&cover_texel(p, false));
                    }
                }
                shot.pixels = Some((rgba, size.0, size.1));
            }
            Err(e) => log::warn!("cover picture not taken: {e:#}"),
        }
    }
}

/// One texel of the reward's read-back as RGBA, the shot's own alpha kept
/// (0x5C3130: StretchRect of the back buffer into the
/// A8R8G8B8 capture target copies the alpha too; the clone wrote 255).
fn cover_texel(p: &[u8], bgra: bool) -> [u8; 4] {
    if bgra { [p[2], p[1], p[0], p[3]] } else { [p[0], p[1], p[2], p[3]] }
}

#[cfg(test)]
mod cover_texel_tests {
    /// 0x5C3130: the capture keeps the back buffer's alpha.
    #[test]
    fn the_cover_read_back_keeps_the_shots_alpha() {
        assert_eq!(super::cover_texel(&[1, 2, 3, 77], true), [3, 2, 1, 77]);
        assert_eq!(super::cover_texel(&[1, 2, 3, 0], false), [1, 2, 3, 0]);
    }
}
