use super::*;

impl Game {
    /// A picture: the frame drawn again offscreen at the Picture Size's
    /// multiple of the display (held by [`ph::capture_scale`]; the Graphics
    /// page's SSAA / MSAA on top of it), no help bar, written as PNG to the
    /// Screenshots folder on a thread. The frame counter is put back (a
    /// capture is no game frame). Returns the picture's size. An error
    /// (the draw, or a window format the read-back cannot convert) leaves
    /// the window as it was; [`Game::photo_key`] shows it under the strip.
    pub(crate) fn photo_capture(&mut self) -> Result<Option<(u32, u32)>> {
        let Some(scale) = self.photo.as_ref().map(|p| p.look.capture) else { return Ok(None) };
        let Some(w) = self.platform.window.as_mut() else { return Ok(None) };
        w.settle();
        let out = (w.config.width, w.config.height);
        let samples = nfsu2::render::display::msaa_samples(&self.settings.custom, &w.gpu, &nfsu2::render::RenderOptions::in_force());
        let k = ph::capture_scale(scale, out, samples, w.gpu.device.limits().max_texture_dimension_2d);
        let size = (out.0 * k, out.1 * k);
        let format = w.config.format;
        let texture = w.gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("photo capture"),
            size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let surface = w.surface.take();
        let offscreen = std::mem::replace(&mut w.offscreen, Some(texture));
        let depth = std::mem::replace(&mut w.depth, create_depth(&w.gpu, size.0, size.1));
        w.config.width = size.0;
        w.config.height = size.1;
        nfsu2::qol::set_output_size(size);
        let frame_count = self.pacing.frame_count;
        if let Some(p) = self.photo.as_mut() {
            p.capturing = true;
        }
        let drawn = self.draw();
        if let Some(p) = self.photo.as_mut() {
            p.capturing = false;
        }
        self.pacing.frame_count = frame_count;
        let w = self.platform.window.as_mut().unwrap();
        w.settle();
        let texture = std::mem::replace(&mut w.offscreen, offscreen).unwrap();
        w.surface = surface;
        w.depth = depth;
        w.config.width = out.0;
        w.config.height = out.1;
        nfsu2::qol::set_output_size(out);
        let pixels = drawn.and_then(|()| nfsu2::render::read_back_rgba8(&w.gpu, &texture));
        drop(texture);
        if let Some(post) = w.post.as_mut() {
            // the capture's big targets go now (the next frame makes its own)
            post.set_output_size(out);
        }
        if let Some(g) = self.photo.as_ref().and_then(|p| self.render.photo_grades.get_mut(&p.id)) {
            g.release();
        }
        let mut pixels = pixels?;
        for px in pixels.chunks_exact_mut(4) {
            px[3] = 255;
        }
        let Some(p) = self.photo.as_mut() else { return Ok(None) };
        let dir = p.dir.clone();
        let stamp = nfsu2::qol::saves::timestamp(std::time::SystemTime::now());
        p.saving.push(std::thread::spawn(move || write_picture(&dir, &stamp, &pixels, size)));
        Ok(Some(size))
    }

    /// The pictures still being written, written (a headless run's end).
    #[cfg(feature = "dev-harness")]
    pub(crate) fn photo_flush(&mut self) -> Vec<std::path::PathBuf> {
        let Some(p) = self.photo.as_mut() else { return Vec::new() };
        std::mem::take(&mut p.saving).into_iter().filter_map(written).collect()
    }
}
