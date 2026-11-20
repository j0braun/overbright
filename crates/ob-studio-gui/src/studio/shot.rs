//! Studio drawn without a window: a few frames at a size (the layout settles), then, for `--shot`, the last one
//! rendered off-screen and written as a PNG; [`frames`] alone runs headless (no GPU) for the tests.

use std::path::Path;

use anyhow::Result;
use nfsu2::studio_api::render::Gpu;

use super::Studio;
use super::layout::Layout;

/// The raw input of frame `i` of a `w` × `h` pixel window at `scale` (`events` in the first frame).
pub fn raw_input(size: (u32, u32), scale: f32, i: usize, events: Vec<egui::Event>) -> egui::RawInput {
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(size.0 as f32 / scale, size.1 as f32 / scale));
    let mut raw = egui::RawInput {
        screen_rect: Some(rect),
        time: Some(i as f64 / 60.0),
        predicted_dt: 1.0 / 60.0,
        focused: true,
        events: if i == 0 { events } else { vec![] },
        ..Default::default()
    };
    raw.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(scale);
    raw
}

/// Run `n` frames headless; the last frame's output and layout.
pub fn frames(s: &mut Studio, size: (u32, u32), scale: f32, n: usize) -> (egui::FullOutput, Layout) {
    s.fixed_clock = true;
    if let Some(h) = s.editor.as_mut() {
        h.app.fixed_clock = true;
    }
    let mut out = None;
    for i in 0..n.max(1) {
        out = Some(s.frame(raw_input(size, scale, i, vec![])));
    }
    let layout = s.last.clone().unwrap_or_else(|| super::layout::layout(1.0, 1.0, &s.want()));
    (out.unwrap_or_default(), layout)
}

/// Draw `s` off-screen into `out` (`size` pixels at `scale`).
pub fn shot(s: &mut Studio, gpu: &Gpu, size: (u32, u32), scale: f32, out: &Path) -> Result<Layout> {
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut egui_r = egui_wgpu::Renderer::new(
        &gpu.device,
        format,
        egui_wgpu::RendererOptions {
            msaa_samples: 1,
            depth_stencil_format: None,
            dithering: false,
            predictable_texture_filtering: true,
        },
    );
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("studio shot"),
        size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    s.fixed_clock = true;
    if let Some(h) = s.editor.as_mut() {
        h.app.fixed_clock = true;
        h.app.panels.viewport.fix_clock(crate::editor::app::SHOT_CLOCK);
    }
    for i in 0..4 {
        let full = s.frame(raw_input(size, scale, i, vec![]));
        let bufs = paint(s, gpu, &mut egui_r, &view, size, full);
        gpu.queue.submit(bufs);
    }
    let bytes_per_row = (size.0 * 4).div_ceil(256) * 256;
    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("studio shot readback"),
        size: u64::from(bytes_per_row * size.1),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(bytes_per_row), rows_per_image: None },
        },
        wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
    );
    gpu.queue.submit([encoder.finish()]);
    let slice = readback.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    gpu.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None })?;
    let mapped = slice.get_mapped_range()?;
    let mut pixels = Vec::with_capacity((size.0 * size.1 * 4) as usize);
    for row in mapped.chunks(bytes_per_row as usize).take(size.1 as usize) {
        pixels.extend_from_slice(&row[..(size.0 * 4) as usize]);
    }
    if let Some(d) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(d)?;
    }
    image::save_buffer(out, &pixels, size.0, size.1, image::ColorType::Rgba8)?;
    Ok(s.last.clone().expect("a frame was drawn"))
}

/// Paint one frame's output into `target`: textures uploaded, the page cleared, egui drawn.
pub fn paint(
    s: &mut Studio,
    gpu: &Gpu,
    egui_r: &mut egui_wgpu::Renderer,
    target: &wgpu::TextureView,
    size: (u32, u32),
    full: egui::FullOutput,
) -> Vec<wgpu::CommandBuffer> {
    let mut full = full;
    // Every frame's texture changes since the last paint, in order.
    let mut textures = std::mem::take(&mut s.textures.0);
    textures.append(std::mem::take(&mut full.textures_delta));
    for (id, deltas) in &textures.set {
        for delta in deltas {
            egui_r.update_texture(&gpu.device, &gpu.queue, *id, delta);
        }
    }
    let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("studio") });
    // World and Races: the map editor's view, rendered into its texture before egui draws it.
    s.render_view(gpu, egui_r, &mut encoder);
    let jobs = s.ctx.tessellate(full.shapes, full.pixels_per_point);
    let screen =
        egui_wgpu::ScreenDescriptor { size_in_pixels: [size.0, size.1], pixels_per_point: full.pixels_per_point };
    let mut out = egui_r.update_buffers(&gpu.device, &gpu.queue, &mut encoder, &jobs, &screen);
    {
        let page = crate::editor::theme::tokens().page;
        let mut pass = encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("studio egui"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: f64::from(page.0) / 255.0,
                            g: f64::from(page.1) / 255.0,
                            b: f64::from(page.2) / 255.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            })
            .forget_lifetime();
        egui_r.render(&mut pass, &jobs, &screen);
    }
    out.push(encoder.finish());
    for id in &textures.free {
        egui_r.free_texture(id);
    }
    textures.clear();
    out
}
