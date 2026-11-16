//! GPU byte-equivalence against the pre-specialization shader, plus optional
//! isolated pass timestamps. No game assets required. Run with --bench for timing.
use anyhow::{Result, ensure};
use nfsu2::render::{Gpu, enhance::{Enhance, Plan}};
use wgpu::util::DeviceExt;

fn main() -> Result<()> {
    nfsu2::prof::gpu::want(true);
    let gpu = pollster::block_on(Gpu::new(None, wgpu::Instance::default()))?;
    println!("Adapter: {:?}", gpu.adapter.get_info());
    let d = &gpu.device;
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let reference = d.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("frozen resample reference"), source: wgpu::ShaderSource::Wgsl(include_str!("fixtures/resample-reference.wgsl").into()),
    });
    let pipeline = d.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("reference"), layout: None,
        vertex: wgpu::VertexState { module: &reference, entry_point: Some("vs_main"), compilation_options: Default::default(), buffers: &[] },
        fragment: Some(wgpu::FragmentState { module: &reference, entry_point: Some("fs_resample"), compilation_options: Default::default(), targets: &[Some(format.into())] }),
        primitive: Default::default(), depth_stencil: None, multisample: Default::default(), multiview_mask: None, cache: None,
    });
    let sampler = d.create_sampler(&wgpu::SamplerDescriptor { mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, ..Default::default() });
    let texture = |size: (u32,u32)| d.create_texture(&wgpu::TextureDescriptor {
        label: Some("equivalence"), size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
        mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC, view_formats: &[],
    });
    let mut enhance = Enhance::new(&gpu, format);
    let bench = std::env::args().any(|s| s == "--bench");
    let mut cases = vec![];
    for out in [(1,1),(7,5),(31,19),(257,129),(1920,1080),(3840,2160)] {
        for n in [2,3,4] { cases.push(((out.0*n,out.1*n),out)); }
    }
    for percent in [50, 67, 75, 100, 125, 141, 150, 173, 175, 225, 250, 283, 350] {
        let out = (31,19);
        let src = ((out.0 * percent + 50) / 100, (out.1 * percent + 50) / 100);
        if src != out { cases.push((src,out)); }
    }
    cases.extend([((47,29),(31,19)),((93,58),(31,19)),((123,75),(31,19)),((11,7),(31,19)),((62,57),(31,19)),((62,38),(31,19))]);
    for (src,out) in cases {
        let source = texture(src);
        // Deterministic noise in all four channels plus sharp borders and impulses.
        let mut state = 123456789u32;
        let mut pixels = Vec::with_capacity((src.0*src.1*4) as usize);
        for y in 0..src.1 { for x in 0..src.0 { for c in 0..4 {
            state ^= state << 13; state ^= state >> 17; state ^= state << 5;
            pixels.push(if x==0 || y==0 || x==src.0-1 || y==src.1-1 { if (x+y+c)%2==0 {255} else {0} } else { state as u8 });
        } } }
        gpu.queue.write_texture(wgpu::TexelCopyTextureInfo { texture: &source, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All }, &pixels,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(src.0*4), rows_per_image: Some(src.1) }, source.size());
        drop(pixels);
        let source_view = source.create_view(&Default::default());
        let expected = texture(out); let actual = texture(out);
        let expected_view = expected.create_view(&Default::default()); let actual_view = actual.create_view(&Default::default());
        let uniform: [f32;8] = [1.0/src.0 as f32,1.0/src.1 as f32,src.0 as f32,src.1 as f32,0.0,src.0 as f32/out.0 as f32,src.1 as f32/out.1 as f32,0.0];
        let buffer = d.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(&uniform), usage: wgpu::BufferUsages::UNIFORM });
        let group = d.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &pipeline.get_bind_group_layout(0), entries: &[
            wgpu::BindGroupEntry { binding:0, resource:wgpu::BindingResource::TextureView(&source_view) },
            wgpu::BindGroupEntry { binding:1, resource:wgpu::BindingResource::Sampler(&sampler) },
            wgpu::BindGroupEntry { binding:2, resource:buffer.as_entire_binding() },
        ] });
        let draw_reference = |encoder: &mut wgpu::CommandEncoder| {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor { label:Some("reference"), color_attachments:&[Some(wgpu::RenderPassColorAttachment { view:&expected_view,depth_slice:None,resolve_target:None,ops:wgpu::Operations {load:wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),store:wgpu::StoreOp::Store} })],depth_stencil_attachment:None,timestamp_writes:None,occlusion_query_set:None,multiview_mask:None });
            pass.set_pipeline(&pipeline);pass.set_bind_group(0,&group,&[]);pass.draw(0..3,0..1);
        };
        let plan = Plan { fxaa:false,smaa:false,sharpen:0.0,fsr:false };
        let mut encoder = d.create_command_encoder(&Default::default());
        draw_reference(&mut encoder);
        enhance.run(&gpu,&mut encoder,&source_view,src,&actual_view,out,plan);
        gpu.queue.submit([encoder.finish()]);
        let a=nfsu2::render::read_back(&gpu,&expected,out.0,out.1);
        let b=nfsu2::render::read_back(&gpu,&actual,out.0,out.1);
        let count=a.iter().zip(&b).filter(|(a,b)|a!=b).count();
        let max=a.iter().zip(&b).map(|(a,b)|a.abs_diff(*b)).max().unwrap();
        println!("{src:?}->{out:?}: differing bytes {count}, max delta {max}");
        ensure!(count==0,"GPU output differs");
        if bench && out.0>=1920 {
            // Paired alternating runs include an unmeasured warmup, then 5 pairs.
            // Submission completion brackets each sample; no overlap with other work.
            ensure!(d.features().contains(nfsu2::prof::gpu::FEATURES), "GPU timestamps required for benchmark");
            let queries=d.create_query_set(&wgpu::QuerySetDescriptor {label:None,ty:wgpu::QueryType::Timestamp,count:64});
            let resolved=d.create_buffer(&wgpu::BufferDescriptor {label:None,size:512,usage:wgpu::BufferUsages::QUERY_RESOLVE|wgpu::BufferUsages::COPY_SRC,mapped_at_creation:false});
            let readback=d.create_buffer(&wgpu::BufferDescriptor {label:None,size:512,usage:wgpu::BufferUsages::MAP_READ|wgpu::BufferUsages::COPY_DST,mapped_at_creation:false});
            for round in 0..6 { for optimized in if round%2==0 {[false,true]} else {[true,false]} {
                let start=std::time::Instant::now();
                for sample in 0..32 {
                    let mut encoder=d.create_command_encoder(&Default::default());
                    encoder.write_timestamp(&queries,sample*2);
                    if optimized { enhance.run(&gpu,&mut encoder,&source_view,src,&actual_view,out,plan); } else {draw_reference(&mut encoder);}
                    encoder.write_timestamp(&queries,sample*2+1);
                    gpu.queue.submit([encoder.finish()]);
                }
                let mut encoder=d.create_command_encoder(&Default::default());
                encoder.resolve_query_set(&queries,0..64,&resolved,0);
                encoder.copy_buffer_to_buffer(&resolved,0,&readback,0,512);
                gpu.queue.submit([encoder.finish()]);
                readback.slice(..).map_async(wgpu::MapMode::Read, |_| {});
                d.poll(wgpu::PollType::Wait {submission_index:None,timeout:None})?;
                let wall_ms=start.elapsed().as_secs_f64()*1000.0/32.0;
                let mapped=readback.slice(..).get_mapped_range()?;
                let times: &[u64]=bytemuck::cast_slice(&mapped);
                let gpu_ms=times.chunks_exact(2).map(|t|(t[1]-t[0]) as f64).sum::<f64>()*gpu.queue.get_timestamp_period() as f64/32.0/1_000_000.0;
                drop(mapped);readback.unmap();
                if round>0 {println!("TIMING src={src:?} out={out:?} optimized={optimized} round={round} wall_ms={wall_ms:.6} gpu_ms={gpu_ms:.6}");}
            } }
        }
    }
    Ok(())
}
