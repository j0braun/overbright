//! Software-GPU check aid (docs/developing/software-gpu.md): the light-streak
//! decay's blend (ZERO / CONSTANT on an Rgba8Unorm target, 0x5CBB6A) on
//! whatever adapter Vulkan gives (e.g. VK_DRIVER_FILES=lavapipe's ICD);
//! prints the texel a white target keeps after one, two and four decays.
use wgpu::util::DeviceExt;

fn main() {
    pollster::block_on(run());
}

async fn run() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends: wgpu::Backends::VULKAN, ..wgpu::InstanceDescriptor::new_without_display_handle() });
    let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions::default()).await.expect("adapter");
    println!("adapter: {}", adapter.get_info().name);
    let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor::default()).await.expect("device");
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let tex = device.create_texture(&wgpu::TextureDescriptor { label: None, size: wgpu::Extent3d { width: 64, height: 64, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format, usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC, view_formats: &[] });
    let view = tex.create_view(&Default::default());
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl("@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> { let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u)); return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0); }\n@fragment fn fs() -> @location(0) vec4<f32> { return vec4<f32>(0.0); }\n@fragment fn fs_add() -> @location(0) vec4<f32> { return vec4<f32>(0.25, 0.25, 0.25, 0.5); }".into()) });
    let blend = wgpu::BlendState { color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::Zero, dst_factor: wgpu::BlendFactor::Constant, operation: wgpu::BlendOperation::Add }, alpha: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::Zero, dst_factor: wgpu::BlendFactor::Constant, operation: wgpu::BlendOperation::Add } };
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: None,
        vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
        fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs"), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format, blend: Some(blend), write_mask: wgpu::ColorWrites::ALL })] }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    let add_blend = if std::env::args().any(|a| a == "--constant") {
        let c = wgpu::BlendComponent { src_factor: wgpu::BlendFactor::Constant, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add };
        wgpu::BlendState { color: c, alpha: c }
    } else {
        wgpu::BlendState { color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::SrcAlpha, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add }, alpha: wgpu::BlendComponent::OVER }
    };
    let depth = device.create_texture(&wgpu::TextureDescriptor { label: None, size: wgpu::Extent3d { width: 64, height: 64, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Depth32Float, usage: wgpu::TextureUsages::RENDER_ATTACHMENT, view_formats: &[] });
    let depth_view = depth.create_view(&Default::default());
    let add = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: None,
        vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
        fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs_add"), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format, blend: Some(add_blend), write_mask: wgpu::ColorWrites::COLOR })] }),
        primitive: Default::default(),
        depth_stencil: Some(wgpu::DepthStencilState { format: wgpu::TextureFormat::Depth32Float, depth_write_enabled: Some(false), depth_compare: Some(wgpu::CompareFunction::LessEqual), stencil: Default::default(), bias: Default::default() }),
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    let out = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: &[0u8; 256 * 64], usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ });
    let decay = wgpu::Color { r: 120.0 / 255.0, g: 90.0 / 255.0, b: 170.0 / 255.0, a: 1.0 };
    for n in [1, 2, 4] {
        let mut enc = device.create_command_encoder(&Default::default());
        {
            let _clear = enc.begin_render_pass(&wgpu::RenderPassDescriptor { label: None, color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &view, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::WHITE), store: wgpu::StoreOp::Store } })], depth_stencil_attachment: None, timestamp_writes: None, occlusion_query_set: None, multiview_mask: None });
        }
        for _ in 0..n {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor { label: None, color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &view, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } })], depth_stencil_attachment: None, timestamp_writes: None, occlusion_query_set: None, multiview_mask: None });
            pass.set_pipeline(&pipeline);
            pass.set_blend_constant(decay);
            pass.draw(0..3, 0..1);
            drop(pass);
            if std::env::args().any(|a| a == "--add") {
                let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor { label: None, color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &view, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } })], depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &depth_view, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }), stencil_ops: None }), timestamp_writes: None, occlusion_query_set: None, multiview_mask: None });
                pass.set_pipeline(&add);
                pass.set_blend_constant(wgpu::Color { r: 22.0 / 255.0, g: 22.0 / 255.0, b: 22.0 / 255.0, a: 22.0 / 255.0 });
                pass.draw(0..3, 0..1);
            }
        }
        enc.copy_texture_to_buffer(tex.as_image_copy(), wgpu::TexelCopyBufferInfo { buffer: &out, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(256), rows_per_image: None } }, wgpu::Extent3d { width: 64, height: 64, depth_or_array_layers: 1 });
        queue.submit([enc.finish()]);
        out.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let px: Vec<u8> = out.slice(..).get_mapped_range().expect("mapped")[..4].to_vec();
        out.unmap();
        println!("after {n} decay(s): {px:?}");
    }
}
