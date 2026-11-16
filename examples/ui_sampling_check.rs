//! GPU regression for the front end's texture sampling, through the real
//! UI renderer and shader (not a CPU copy of the shader).
//! * Without an HD UI Art pack: a game texture samples as fe.efx does
//!   (bilinear, the same as the fill path, which never sharpens), so HD UI
//!   Art off is native at every size.
//! * With a pack (the example re-runs itself with `NFSU2_TEXTURE_PACK` set
//!   to a one-texture pack): pack art drawn magnified goes through
//!   ui.wgsl `sharp` — the pixels in the middle of a texel are the
//!   texel's own colour — while the same art drawn as a fill stays plain bilinear.
use anyhow::{Result, ensure};
use glam::Vec2;
use nfsu2::{frontend::{Assets, Quad}, render::{Gpu, ui::UiRenderer, display::Place}, formats::texture::{Texture, PixelFormat, Blend}};

const HASH: u32 = 0xDEAD1234;
/// The fixture (8 × 8): a diagonal of two colours.
fn texel(x: u32, y: u32) -> [u8; 4] {
    if x <= y { [240, 40, 90, 255] } else { [30, 180, 230, 255] }
}

fn main() -> Result<()> {
    let root = std::path::PathBuf::from(std::env::var("NFSU2_ROOT")?);
    let pack = std::env::args().nth(1).as_deref() == Some("--pack");
    if !pack {
        ensure!(!nfsu2::hd_ui_pack::active(), "run without NFSU2_TEXTURE_PACK");
    }
    let mut assets = Assets::load(&root)?;
    let data: Vec<u8> = (0..64).flat_map(|i| texel(i % 8, i / 8)).collect();
    assets.textures.insert(HASH, Texture { name: "sampling fixture".into(), hash: HASH, width: 8, height: 8, format: PixelFormat::Rgba8, mip_count: 1, data: data.into(), palette: vec![], alpha_test: false, blend: Blend::Alpha, rendering_order: 0, late_buckets: false, scroll: Default::default() });
    let gpu = pollster::block_on(Gpu::new(None, wgpu::Instance::default()))?;
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut ui = UiRenderer::new(&gpu, format);
    for size in [(1280, 960), (1920, 1080), (3840, 2160)] {
        let target = gpu.device.create_texture(&wgpu::TextureDescriptor { label: Some("sampling check"), size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format, usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC, view_formats: &[] });
        let view = target.create_view(&Default::default());
        // 24 canvas units for the 8 texels (3×, under `FILL_MAGNIFY`, so
        // not a fill): the pack's 16 × 16 art 1.5 units a texel, 3–6.75
        // pixels.
        let (a, b) = (Vec2::new(308., 228.), Vec2::new(332., 252.));
        let quad = Quad { texture: Some(HASH), rect: [a, b], uv: [Vec2::ZERO, Vec2::ONE], color: [1.; 4], z: 0., angle: 0., place: Place::CENTRE };
        let mut outputs = Vec::new();
        for fill in [false, true] {
            let mut q = quad;
            q.place.fill = fill;
            ui.render(&gpu, &assets, &view, size, &[q], true);
            outputs.push(nfsu2::render::read_back(&gpu, &target, size.0, size.1));
        }
        if !pack {
            ensure!(outputs[0] == outputs[1], "without a pack the art is not native bilinear at {size:?}");
            println!("{size:?}: no pack: art samples native bilinear (equal to the fill path)");
            continue;
        }
        ensure!(outputs[0] != outputs[1], "pack art is not sharpened at {size:?}");
        // The pack art is the fixture enlarged 2× (16 × 16). Each target
        // pixel inside the quad: its UV (the native half-texel inset of
        // the 8 × 8 texture, 0x5C5030, and the half-pixel shift of
        // `UiRenderer::record`); where that lies in the middle of a pack
        // texel (0.3–0.7 on both axes), sharp bilinear gives the texel's
        // own colour, which plain bilinear does not.
        let frame = nfsu2::render::display::UiFrame::new(size);
        let half = Vec2::new(0.5 / frame.scale.0, 0.5 / frame.scale.1);
        let (p0, p1) = (frame.to_pixels(a + half, Place::CENTRE), frame.to_pixels(b + half, Place::CENTRE));
        let (inset, span) = (0.5 / 8.0, 1.0 - 1.0 / 8.0);
        let (mut worst, mut checked, mut worst_fill) = (0u8, 0u32, 0u8);
        for py in p0.y.floor() as u32..p1.y.ceil() as u32 {
            for px in p0.x.floor() as u32..p1.x.ceil() as u32 {
                let f = (Vec2::new(px as f32 + 0.5, py as f32 + 0.5) - p0) / (p1 - p0);
                if f.min_element() <= 0.0 || f.max_element() >= 1.0 {
                    continue;
                }
                let t = (Vec2::splat(inset) + f * span) * 16.0;
                let fr = t.fract();
                if fr.min_element() < 0.3 || fr.max_element() > 0.7 {
                    continue;
                }
                let want = texel(t.x as u32 / 2, t.y as u32 / 2);
                let i = ((py * size.0 + px) * 4) as usize;
                for ch in 0..3 {
                    worst = worst.max(outputs[0][i + ch].abs_diff(want[ch]));
                    worst_fill = worst_fill.max(outputs[1][i + ch].abs_diff(want[ch]));
                }
                checked += 1;
            }
        }
        ensure!(checked > 0 && worst <= 1, "pack texel middles off by {worst} ({checked} pixels) at {size:?}");
        println!("{size:?}: pack: art sharp-bilinear, {checked} texel-middle pixels within {worst} of their texel (the fill path, plain bilinear: {worst_fill})");
    }
    if !pack {
        // The pack run, in a process of its own (the pack is chosen once).
        let dir = std::env::temp_dir().join(format!("nfsu2-ui-sampling-{}", std::process::id()));
        std::fs::create_dir_all(&dir)?;
        image::RgbaImage::from_fn(16, 16, |x, y| image::Rgba(texel(x / 2, y / 2))).save(dir.join(format!("{HASH:08X}.png")))?;
        let status = std::process::Command::new(std::env::current_exe()?).arg("--pack").env("NFSU2_TEXTURE_PACK", &dir).status()?;
        let _ = std::fs::remove_dir_all(&dir);
        ensure!(status.success(), "the pack run failed");
    }
    Ok(())
}
