//! Real-asset GPU regression: enhanced reflections change shading and
//! returning to native restores identical pixels in the same renderer.
use anyhow::{Result, ensure};
use glam::{Mat4, Vec3};
use nfsu2::{car::{Car, WheelPose}, qol::Enhancements, render::{self, Gpu, Camera, car::{CarRenderer, static_envmap}, lighting::{CarLighting, DEFAULT_RIG}}};
fn main() -> Result<()> {
    // A developer tool: the developer cache, never the player's (docs/developing/asset-cache.md).
    nfsu2::platform::dirs::developer_program();
    let gpu = pollster::block_on(Gpu::new(None, wgpu::Instance::default()))?;
    let root = nfsu2::game_root();
    let car = Car::load(&root,"PEUGOT")?;
    let mut renderer = CarRenderer::new(&gpu,&car,wgpu::TextureFormat::Rgba8Unorm,&nfsu2::render::RenderOptions::in_force());
    let platform = nfsu2::garage::Platform::load(&root,"SHOWROOM")?;
    if let Some(faces) = &platform.envmap {
        if let Some(cube) = static_envmap(&gpu,faces) { renderer.set_environment(&gpu,&cube); }
    }
    let size = (320,240);
    let color = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("car reflection regression"), size: wgpu::Extent3d { width:size.0,height:size.1,depth_or_array_layers:1 },
        mip_level_count:1,sample_count:1,dimension:wgpu::TextureDimension::D2,format:wgpu::TextureFormat::Rgba8Unorm,
        usage:wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,view_formats:&[],
    });
    let view = color.create_view(&Default::default());
    let depth = render::create_depth(&gpu,size.0,size.1);
    let camera = Camera::looking_at(Vec3::new(5.0,-7.0,2.5),Vec3::new(0.0,0.0,0.4));
    let lighting = CarLighting::frontend(&DEFAULT_RIG,&camera,Mat4::IDENTITY);
    let mut frames=Vec::new();
    for enabled in [false,true,false] {
        Enhancements { enhanced_reflections:enabled,..Default::default() }.apply();
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label:Some("clear fixture"), color_attachments:&[Some(wgpu::RenderPassColorAttachment { view:&view,depth_slice:None,resolve_target:None,ops:wgpu::Operations { load:wgpu::LoadOp::Clear(wgpu::Color::BLACK),store:wgpu::StoreOp::Store } })],
                depth_stencil_attachment:Some(wgpu::RenderPassDepthStencilAttachment { view:&depth,depth_ops:Some(wgpu::Operations { load:wgpu::LoadOp::Clear(1.0),store:wgpu::StoreOp::Store }),stencil_ops:None }),timestamp_writes:None,occlusion_query_set:None,multiview_mask:None,
            });
        }
        renderer.render(&gpu,&mut encoder,&view,&depth,camera.view_projection(size.0 as f32 / size.1 as f32),camera.view(),&lighting,Mat4::IDENTITY,&[WheelPose::default();4], &nfsu2::render::RenderOptions::in_force());
        gpu.queue.submit([encoder.finish()]);
        frames.push(render::read_back(&gpu,&color,size.0,size.1));
    }
    ensure!(frames[0].chunks_exact(4).filter(|p| p[0] != 0 || p[1] != 0 || p[2] != 0).count()>100,"fixture must draw a car");
    let changed=frames[0].chunks_exact(4).zip(frames[1].chunks_exact(4)).filter(|(a,b)| a!=b).count();
    ensure!(changed>100,"enabled reflection shading must reach real car pixels; changed {changed}");
    ensure!(frames[0]==frames[2],"Native must restore identical pixels after enhanced mode");
    println!("Car reflection GPU check: {changed} changed pixels, native restoration exact");
    Ok(())
}
