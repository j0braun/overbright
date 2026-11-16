//! Exercise SSAA through the real GPU downsample path. The 4x fixture
//! averages four distinct samples per output pixel; 2x preserves a flat
//! image at fractional dimensions. No game assets required.
use anyhow::{Result, ensure};
use nfsu2::{qol::{self, Enhancements}, render::{Gpu, enhance::{Enhance, Plan}}};

fn main() -> Result<()> {
    let gpu = pollster::block_on(Gpu::new(None, wgpu::Instance::default()))?;
    if !qol::aa_offered(7, (4, 4)) {
        println!("SKIPPED: this adapter intentionally disables supersampling");
        return Ok(());
    }
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut enhance = Enhance::new(&gpu, format);
    let out = (4, 4);
    let texture = |size: (u32, u32)| gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("SSAA regression"),
        size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
        mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    for (aa, expected_size) in [(7, (8, 8)), (6, (6, 6)), (9, (12, 12)), (10, (16, 16)), (7, (8, 8))] {
        Enhancements { post_aa: aa, ..Default::default() }.apply();
        let mut custom = [0; 22];
        custom[14] = 3;
        ensure!(nfsu2::render::display::msaa_samples(&custom, &gpu, &nfsu2::render::RenderOptions::in_force()) == 1, "native MSAA must not stack with SSAA");
        let size = qol::scene_size(out, 1);
        ensure!(size == expected_size, "SSAA {aa}: wrong scene size {size:?}");
        let source = texture(size);
        let pixels: Vec<u8> = (0..size.1).flat_map(|y| (0..size.0).flat_map(move |x| {
            if aa == 6 { [60, 120, 180, 255] } else if aa >= 9 {
                let side = if aa == 9 { 3 } else { 4 };
                // Only top-left sample lights: old sparse four-tap kernel misses
                // or misweights this at odd/fourfold scales.
                if x % side == 0 && y % side == 0 { [240, 120, 60, 255] } else { [0, 0, 0, 255] }
            } else {
                let base = ((y / 2 * 4 + x / 2) * 12) as u8;
                let sample = ((y % 2 * 2 + x % 2) * 4) as u8;
                [base + sample, 40 + sample, 80 + sample, 255]
            }
        })).collect();
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &source, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &pixels,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(size.0 * 4), rows_per_image: Some(size.1) },
            source.size(),
        );
        let target = texture(out);
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        let plan = Plan::of(&nfsu2::render::RenderOptions::in_force());
        ensure!(!plan.fxaa && !plan.smaa && plan.sharpen == 0.0);
        ensure!(plan.any(size, out));
        enhance.run(&gpu, &mut encoder, &source.create_view(&Default::default()), size, &target.create_view(&Default::default()), out, plan);
        gpu.queue.submit([encoder.finish()]);
        let actual = nfsu2::render::read_back(&gpu, &target, out.0, out.1);
        for (i, pixel) in actual.chunks_exact(4).enumerate() {
            let expected = if aa == 6 { [60, 120, 180, 255] } else if aa == 9 { [27, 13, 7, 255] } else if aa == 10 { [15, 8, 4, 255] } else { [i as u8 * 12 + 6, 46, 86, 255] };
            ensure!(pixel.iter().zip(expected).all(|(&got, want)| got.abs_diff(want) <= 1), "SSAA {aa} pixel {i}: {pixel:?}, expected {expected:?}");
        }
        println!("{}: {size:?} -> {out:?}, GPU pixels match", qol::aa_name(aa).unwrap());
    }
    // Exercise composed filters, with and without scaling/sharpening, while
    // reusing the same Enhance instance across sizes and mode changes.
    for size in [(32,32), (16,16), (48,48), (16,16)] {
        let source = texture(size);
        let pixels: Vec<u8> = (0..size.1).flat_map(|y| (0..size.0).flat_map(move |x| {
            let c = if x * 3 > y * 2 + 4 { 230 } else { 20 };
            [c,c,c,255]
        })).collect();
        gpu.queue.write_texture(wgpu::TexelCopyTextureInfo { texture: &source, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All }, &pixels,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(size.0 * 4), rows_per_image: Some(size.1) }, source.size());
        let out = (16,16);
        for sharpen in [0.0, 1.0] {
            let mut images = Vec::new();
            for (fxaa,smaa) in [(false,false), (true,false), (false,true), (true,true)] {
                let target = texture(out);
                let mut encoder = gpu.device.create_command_encoder(&Default::default());
                enhance.run(&gpu, &mut encoder, &source.create_view(&Default::default()), size, &target.create_view(&Default::default()), out, Plan { fxaa,smaa,sharpen,fsr:false });
                gpu.queue.submit([encoder.finish()]);
                let image = nfsu2::render::read_back(&gpu,&target,out.0,out.1);
                ensure!(image.chunks_exact(4).all(|p| p[3] == 255));
                images.push(image);
            }
            ensure!(images[3] != images[2], "combined AA must actually execute FXAA after SMAA");
            ensure!(images[3] != images[1], "combined AA must actually execute SMAA before FXAA");
        }
    }
    let mut custom = [0;22]; custom[14] = 3;
    for samples in [1,2,4,8,16] {
        Enhancements { post_aa: 10, aa_msaa: samples, aa_filter: 4, ..Default::default() }.apply();
        let effective = nfsu2::render::display::msaa_samples(&custom, &gpu, &nfsu2::render::RenderOptions::in_force());
        ensure!(effective == qol::supported_msaa(samples as u32));
        println!("SSAA 16x + MSAA {samples}x + SMAA + FXAA: effective MSAA {effective}x");
    }
    println!("32 composed filter/resize/sharpen GPU cases passed");
    Enhancements::default().apply();
    ensure!(qol::scene_size(out, 1) == out);
    ensure!(!Plan::of(&nfsu2::render::RenderOptions::in_force()).any(out, out), "Off must bypass enhancement passes");
    println!("Off restores native dimensions and bypasses downsampling");
    Ok(())
}
