//! Photo mode's picture is the same whatever the window's format: the
//! swap chain can be 8-bit BGRA or RGBA, 10-bit or 16-bit float
//! (`display::sdr_surface_format`), and the capture is drawn in that
//! format and read back as RGBA8 (`render::read_back_rgba8`). A real free
//! roam (the player's game files), drawn offscreen.
use super::*;
// The test kit (tests/common/mod.rs, docs/developing/testing.md).
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

const SIZE: (u32, u32) = (320, 240);

/// An offscreen window of `format` (as a swap chain of that format).
fn window(game: &mut Game, format: wgpu::TextureFormat) {
    let gpu = pollster::block_on(Gpu::new(None, Gpu::new_instance())).unwrap();
    let offscreen = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("photo format test target"),
        size: wgpu::Extent3d { width: SIZE.0, height: SIZE.1, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: SIZE.0,
        height: SIZE.1,
        present_mode: wgpu::PresentMode::AutoVsync,
        desired_maximum_frame_latency: 2,
        alpha_mode: wgpu::CompositeAlphaMode::Auto,
        view_formats: vec![],
        color_space: Default::default(),
    };
    let depth = create_depth(&gpu, SIZE.0, SIZE.1);
    let ui = UiRenderer::new(&gpu, format);
    game.platform.window = Some(WindowState::new(None, None, Some(offscreen), config, gpu, depth, ui));
}

/// A 1× picture of the same free-roam moment on a window of `format`, as
/// the PNG written.
fn picture(format: wgpu::TextureFormat) -> Vec<u8> {
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("PHOTO_FORMAT_TEST");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    game.explore(None).unwrap();
    window(&mut game, format);
    for _ in 0..3 {
        game.advance(1.0 / 60.0).unwrap();
        game.draw().unwrap();
    }
    game.photo_open();
    let scratch = testkit::Scratch::new("photo-format");
    let p = game.photo.as_mut().expect("photo mode open");
    p.dir = scratch.path().join("Screenshots");
    p.look.capture = 1;
    let size = game.photo_capture().unwrap().expect("a picture");
    assert_eq!(size, SIZE);
    let written = game.photo_flush();
    assert_eq!(written.len(), 1, "{format:?}: one picture written");
    let png = image::open(&written[0]).unwrap().to_rgba8();
    assert_eq!(png.dimensions(), SIZE);
    png.into_raw()
}

/// The read-back itself is exact for every format (`render::upload`'s
/// `rgba8_tests`); what differs here is the drawing: the frame's last
/// passes blend into the window-format target, at 8 bits on an 8-bit
/// window and at 10 or 16 bits (16-bit float unclamped between blends)
/// on the others, so values round differently and an additive highlight
/// a later pass darkens is not first clipped at white. Measured at
/// 320x240: RGBA exact; 10-bit 98.6 % of the colour channels within 1/255,
/// at most 4/255; 16-bit float 98.7 % within 1/255, under 0.2 % above
/// 4/255 (a few up to ~120/255 in highlights).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_picture_is_the_same_in_every_window_format() {
    testkit::game();
    super::test_isolation::exclusive();
    let reference = picture(wgpu::TextureFormat::Bgra8Unorm);
    assert!(reference.chunks_exact(4).any(|p| p[..3] != [0, 0, 0]), "the picture shows the world");
    let mut failures = Vec::new();
    // (format, largest difference, least share within 1/255, most share above 4/255)
    for (format, worst_allowed, within_1, above_4) in [
        (wgpu::TextureFormat::Rgba8Unorm, 0, 1.0, 0.0),
        (wgpu::TextureFormat::Rgb10a2Unorm, 4, 0.98, 0.0),
        (wgpu::TextureFormat::Rgba16Float, 255, 0.98, 0.002),
    ] {
        let other = picture(format);
        assert!(other.chunks_exact(4).all(|p| p[3] == 255), "{format:?}: opaque");
        let mut histogram = [0usize; 256];
        for (a, b) in reference.chunks_exact(4).zip(other.chunks_exact(4)) {
            for c in 0..3 {
                histogram[usize::from(a[c].abs_diff(b[c]))] += 1;
            }
        }
        let n = histogram.iter().sum::<usize>() as f64;
        let worst = histogram.iter().rposition(|&k| k > 0).unwrap();
        let near = (histogram[0] + histogram[1]) as f64 / n;
        let far = histogram[5..].iter().sum::<usize>() as f64 / n;
        println!(
            "{format:?}: within 1/255 {:.2} %, above 4/255 {:.3} %, at most {worst}/255",
            near * 100.0,
            far * 100.0
        );
        if worst > worst_allowed || near < within_1 || far > above_4 {
            failures.push(format!("{format:?}: within 1/255 {near:.4}, above 4/255 {far:.4}, at most {worst}/255"));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
