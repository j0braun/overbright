use super::*;

/// The detailed path: the feeder and, once drawn, its GPU half.
#[derive(Default)]
pub struct Detail {
    pub feeder: Feeder,
    gpu: Option<GpuSmoke>,
    /// The tier the GPU half was made for.
    tier: u8,
    prepared: bool,
}

impl Detail {
    /// Whether the native smoke quads are replaced this frame (the GPU
    /// half exists and was prepared).
    pub fn replaces(&self) -> bool {
        self.prepared && self.gpu.is_some()
    }

    /// The GPU half [`Detail::prepare`] makes on its first call at `tier`,
    /// made now (a drive's load), and its pipelines for the frame's targets
    /// ([`Detail::warm`]). The same object the first frame would make from
    /// the same world; nothing is drawn or stepped.
    pub fn prebuild(
        &mut self,
        gpu: &Gpu,
        tier: u8,
        library: &Library,
        textures: &std::collections::HashMap<u32, crate::formats::texture::Texture>,
        targets: &[(wgpu::TextureFormat, u32, u32)],
    ) {
        let Some(params) = tier_params(tier) else { return };
        if self.gpu.is_none() || self.tier != tier {
            self.gpu = Some(GpuSmoke::new(gpu, params, library, textures));
            self.tier = tier;
        }
        for &(format, samples, depth_samples) in targets {
            self.warm(gpu, format, samples, depth_samples);
        }
    }

    /// Before the frame's particles: make the GPU half when needed, replay
    /// the queued ticks (spawn and update on the GPU) and set the frame's
    /// view. Returns false on Original (nothing to draw, native quads).
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        tier: u8,
        library: &Library,
        palettes: &[crate::particles::Cubic],
        textures: &std::collections::HashMap<u32, crate::formats::texture::Texture>,
        view: &View,
    ) -> bool {
        self.prepared = false;
        let Some(params) = tier_params(tier) else {
            self.gpu = None;
            self.feeder.ticks.clear();
            return false;
        };
        self.feeder.classify(library);
        if self.gpu.is_none() || self.tier != tier {
            self.gpu = Some(GpuSmoke::new(gpu, params, library, textures));
            self.tier = tier;
        }
        let g = self.gpu.as_mut().unwrap();
        g.tables(gpu, library, palettes);
        let ticks: Vec<Tick> = self.feeder.ticks.drain(..).collect();
        g.simulate(gpu, encoder, &ticks);
        g.frame(gpu, view, self.feeder.time as f32);
        self.prepared = self.feeder.live();
        true
    }

    /// The puffs' pipeline for a [`Detail::draw`] with these
    /// arguments built now (once the GPU half exists), so the first puff
    /// drawn is not a pipeline build. Nothing drawn changes.
    pub fn warm(&self, gpu: &Gpu, format: wgpu::TextureFormat, samples: u32, depth_samples: u32) {
        if let Some(g) = self.gpu.as_ref() {
            if let Some(h) = g.half.as_ref().filter(|_| half_size_on()) {
                g.pipeline(gpu, format, 0, depth_samples);
                h.pipeline(gpu, format, samples, depth_samples > 1);
            } else {
                g.pipeline(gpu, format, samples, depth_samples);
            }
        }
    }

    /// Draw the puffs into `target` (format `format`, `samples` samples),
    /// soft against `depth` (`depth_samples`), with the temporal pass's
    /// viewport when given.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        samples: u32,
        depth: &wgpu::TextureView,
        depth_samples: u32,
        size: (u32, u32),
        viewport: Option<[f32; 4]>,
    ) {
        if !self.replaces() {
            return;
        }
        if let Some(g) = self.gpu.as_ref() {
            g.draw(gpu, encoder, target, format, samples, depth, depth_samples, size, viewport);
        }
    }
}
