/// The frame's input from the renderer.
pub struct Frame<'a> {
    pub camera: &'a crate::render::camera::Camera,
    pub aspect: f32,
    pub reversed: bool,
    pub format: wgpu::TextureFormat,
    pub samples: u32,
    pub visible: &'a [bool],
    pub fog: Option<([f32; 3], [f32; 3])>,
    pub time: f32,
    /// The road's wetness (`Weather::road_wet`).
    pub wet: f32,
    pub lamps: &'a [crate::render::lamps::Lamp],
    pub modern: bool,
    /// The row's tier (0 Off, 1 On, 2 Next-Gen).
    pub tier: u8,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct Uniform {
    pub(super) view_proj: [[f32; 4]; 4],
    pub(super) eye: [f32; 4],
    pub(super) right: [f32; 4],
    pub(super) up: [f32; 4],
    pub(super) forward: [f32; 4],
    pub(super) fog_value: [f32; 4],
    pub(super) fog_color: [f32; 4],
    pub(super) counts: [u32; 4],
    pub(super) lamps: [[f32; 4]; 32],
    pub(super) spots: [[f32; 4]; 24],
    pub(super) points: [[f32; 4]; 16],
}

/// What the world pass draws this frame ([`DrawPlan::draw`]).
#[derive(Clone)]
pub struct DrawPlan {
    pub(super) solid: wgpu::RenderPipeline,
    pub(super) decal: wgpu::RenderPipeline,
    pub(super) group: wgpu::BindGroup,
    pub(super) vertices: wgpu::Buffer,
    pub(super) indices: wgpu::Buffer,
    pub(super) instances: wgpu::Buffer,
    /// (mesh's first index, index count, base vertex), instance range, decal.
    pub(super) runs: Vec<((u32, u32, i32), std::ops::Range<u32>, bool)>,
}

impl DrawPlan {
    /// At the end of the main view's world pass (replaces its pipeline,
    /// group 0 and vertex buffers 0 / 1).
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        if self.runs.is_empty() {
            return;
        }
        pass.set_bind_group(0, &self.group, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_vertex_buffer(1, self.instances.slice(..));
        pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint16);
        for decal in [false, true] {
            pass.set_pipeline(if decal { &self.decal } else { &self.solid });
            for ((first, count, base), inst, _) in self.runs.iter().filter(|r| r.2 == decal) {
                pass.draw_indexed(*first..*first + *count, *base, inst.clone());
            }
        }
    }
}
