use super::*;

/// A frame's view and lights (as `gpu_particles::View`, plus the rain).
#[derive(Clone)]
pub struct View {
    pub camera: Camera,
    pub aspect: f32,
    pub reversed: bool,
    pub pixels_per_metre: f32,
    pub lamps: Vec<(Vec3, f32, Vec3)>,
    pub cars: Vec<(Mat4, [u8; 3])>,
    /// The rain (0..1): wet roads, steam.
    pub wet: f32,
    /// Modern Lighting on (the lamps' physical falloff).
    pub modern: bool,
    /// The cars' underglows as lights in the smoke (car matrix,
    /// linear R, G, B 0..1 × the neon's alpha: [`crate::shadow::neon_rgb`]).
    pub neons: Vec<(Mat4, Vec3)>,
    /// The main view's native fog (FogValue x, y, z; colour), None
    /// with the fog off.
    pub fog: Option<([f32; 3], [f32; 3])>,
    /// Further point lights of the scene (position, radius, colour),
    /// e.g. Modern Lighting's coloured lights near the view; the narrow way
    /// in for other light sources (none yet).
    pub points: Vec<crate::render::volume_fx::Lamp>,
}

/// The Cinematic path: its feeder (the shared [`Feeder`]) and GPU half.
#[derive(Default)]
pub struct Cinematic {
    pub feeder: Feeder,
    gpu: Option<GpuSmoke2>,
    prepared: bool,
}

impl Cinematic {
    /// Whether the native smoke quads are replaced this frame.
    pub fn replaces(&self) -> bool {
        self.prepared && self.gpu.is_some()
    }

    /// docs/notes/graphics/atmosphere.md: this frame's volumetric smoke medium (the
    /// density volume, its min corner, voxel edge and voxels), while the
    /// volumetric tier draws.
    pub fn medium(&self) -> Option<(wgpu::TextureView, glam::Vec3, f32, [u32; 3])> {
        if !self.replaces() {
            return None;
        }
        self.gpu.as_ref()?.volume.as_ref()?.medium()
    }

    /// The GPU half [`Cinematic::prepare`] makes on its first call with the
    /// tier on, made now (a drive's load: its 3-D noise and puff textures
    /// built behind the loading screen, not in the first frame), and its
    /// pipelines for the frame's targets ([`Cinematic::warm`]). The same
    /// object the first frame would make from the same world; nothing is
    /// drawn or stepped.
    #[allow(clippy::too_many_arguments)]
    pub fn prebuild(
        &mut self,
        gpu: &Gpu,
        on: bool,
        library: &Library,
        textures: &std::collections::HashMap<u32, crate::formats::texture::Texture>,
        targets: &[(wgpu::TextureFormat, u32, u32)],
    ) {
        if !on {
            return;
        }
        if self.gpu.is_none() {
            self.gpu = Some(GpuSmoke2::new(gpu, CINEMATIC, library, textures));
        }
        for &(format, samples, depth_samples) in targets {
            self.warm(gpu, format, samples, depth_samples);
        }
    }

    /// Before the frame's particles: make the GPU half when `on`, replay
    /// the queued ticks, set the frame's view. Off: drop everything.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        on: bool,
        library: &Library,
        palettes: &[crate::particles::Cubic],
        textures: &std::collections::HashMap<u32, crate::formats::texture::Texture>,
        view: &View,
    ) -> bool {
        self.prepared = false;
        if !on {
            self.gpu = None;
            self.feeder.ticks.clear();
            return false;
        }
        // (The rain's effect on new smoke is smoothed per tick: `Tick::wet`.)
        if self.gpu.is_none() {
            self.gpu = Some(GpuSmoke2::new(gpu, CINEMATIC, library, textures));
        }
        let g = self.gpu.as_mut().unwrap();
        g.tables(gpu, library, palettes);
        let ticks: Vec<Tick> = self.feeder.ticks.drain(..).collect();
        g.simulate(gpu, encoder, &ticks);
        g.frame(gpu, view, self.feeder.time as f32);
        self.prepared = self.feeder.live();
        // The volumetric look (the grid built once a frame, before any draw).
        if self.prepared
            && let Some((v, fv)) = g.volume.as_mut().zip(g.frame_view.take())
        {
            let lights = crate::render::smoke_volume::scene_lights(
                &view.lamps,
                &view.cars,
                &view.neons,
                &view.points,
                view.camera.position,
                view.modern,
                v.look(),
            );
            v.build(gpu, encoder, fv, &lights, g.written);
        }
        true
    }

    /// The pipelines for a [`Cinematic::draw`] with these arguments built
    /// now (no first-use hitch).
    pub fn warm(&self, gpu: &Gpu, format: wgpu::TextureFormat, samples: u32, depth_samples: u32) {
        if let Some(g) = self.gpu.as_ref() {
            g.composite_pipeline(gpu, format, samples, depth_samples > 1);
            g.downsample_pipeline(gpu, depth_samples > 1);
            if let Some(v) = g.volume.as_ref() {
                v.warm(gpu, format, samples, depth_samples);
            }
        }
    }

    /// Draw the smoke over `target` (format, samples), soft against
    /// `depth` (its samples), with the temporal pass's viewport when given.
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
        viewport: Option<[f32; 4]>,
    ) {
        if !self.replaces() {
            return;
        }
        if let Some(g) = self.gpu.as_ref() {
            g.draw(gpu, encoder, target, format, samples, depth, depth_samples, viewport);
        }
    }
}
