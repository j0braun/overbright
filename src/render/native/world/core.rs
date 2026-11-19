//! What recording the world's views reads, shared with the render threads (moved from render/mod.rs).

use crate::render::*;

/// What recording the world's views reads, shared with the render threads
/// that record them ([`WorldRenderer::render_async`]): fixed once built,
/// but for the instances' cull values and moved flags, which only
/// `move_placement` / `update_placement` change, between frames.
pub struct WorldCore {
    /// Sky Next-Gen (docs/notes/graphics/atmosphere.md): the sky pair's palette rows
    /// (made at every load) and the sky, made the first frame the row is on.
    pub(in crate::render) sky_rows: sky_ng::Rows,
    pub(in crate::render) sky_ng: std::sync::OnceLock<sky_ng::NightSky>,
    /// The Sky row of the frame being recorded (its settings, set where
    /// the frame's views and cube are planned).
    pub(in crate::render) sky_on: std::sync::atomic::AtomicBool,
    /// The pipelines into the scene (the main view, shadows, particles,
    /// glows), at the scene's sample count ([`WorldRenderer::set_samples`]).
    pub(in crate::render) scene: std::sync::RwLock<std::sync::Arc<ScenePipelines>>,
    pub(in crate::render) reflection_pipelines: HashMap<PipelineKey, wgpu::RenderPipeline>,
    pub(in crate::render) main_frame: (wgpu::Buffer, wgpu::BindGroup),
    /// The main frame's group 0 with the lamps' buffers while Dynamic
    /// Lamps or a shadow option is on ([`WorldCore::main_group`]).
    pub(in crate::render) lamp_main: std::sync::RwLock<Option<wgpu::BindGroup>>,
    /// Ambient Detail (docs/notes/graphics/ambient-detail.md): the props the main
    /// view's world pass draws last this frame (None: row off).
    pub(in crate::render) ambient_draw: std::sync::Mutex<Option<ambient_detail::DrawPlan>>,
    /// Modern + RT: the same with the traced scene at binding 11, for
    /// the scene's pipelines only while they are made with that layout
    /// ([`WorldCore::scene_group`]); every other pipeline keeps `lamp_main`.
    pub(in crate::render) lamp_main_rt: std::sync::RwLock<Option<wgpu::BindGroup>>,
    pub(in crate::render) reflection_frame: (wgpu::Buffer, wgpu::BindGroup),
    pub(in crate::render) reflection_effect_frame: (wgpu::Buffer, wgpu::BindGroup),
    /// View 3's frame values (the rear-view mirror).
    pub(in crate::render) rear_frame: (wgpu::Buffer, wgpu::BindGroup),
    pub(in crate::render) env_frames: [(wgpu::Buffer, wgpu::BindGroup); 6],
    pub(in crate::render) env: EnvCube,
    pub(in crate::render) env_multiview: Option<EnvMultiview>,
    /// The cube holds only `clear_environment`'s black (nothing drawn
    /// since), so clearing it again changes nothing.
    pub(in crate::render) env_black: std::sync::atomic::AtomicBool,
    /// Each slot's bind group (empty with `texture_arrays`).
    pub(in crate::render) texture_bind_groups: Vec<wgpu::BindGroup>,
    /// Each slot's scroll parameters.
    pub(in crate::render) texture_scrolls: Vec<[f32; 8]>,
    /// The textures as arrays, when the device has immediates.
    pub(in crate::render) texture_arrays: Option<TextureArrays>,
    /// Surface Detail (clone deviation, docs/notes/graphics/surface-maps.md): per slot its
    /// material and normal map layer.
    pub(in crate::render) surface_slots: surface::SlotSurfaces,
    /// SKYMORPHTEXTURE's group per sky table entry.
    pub(in crate::render) sky_groups: Vec<wgpu::BindGroup>,
    pub(in crate::render) animations: Vec<Animation>,
    /// Bind-group slots of the sky table's eight textures (0x8A07A8).
    pub(in crate::render) sky_table: [usize; 8],
    /// Group 2 while drawing the reflection itself (never sampled there).
    pub(in crate::render) no_reflection: wgpu::BindGroup,
    pub(in crate::render) vertices: wgpu::Buffer,
    pub(in crate::render) indices: wgpu::Buffer,
    pub(in crate::render) instances: wgpu::Buffer,
    pub(in crate::render) draws: Vec<Draw>,
    /// Per instance group (`Draw::group`): a sphere around all its
    /// instances as built, for skipping groups outside a view.
    pub(in crate::render) group_bounds: Vec<(Vec3, f32)>,
    /// The same as the road reflection draws them, each instance with a
    /// matrix mirrored about its own origin (m22 negated; see `vs_common`).
    pub(in crate::render) mirrored_bounds: Vec<(Vec3, f32)>,
    /// Per group: an instance of it has moved since (`move_placement`,
    /// `update_placement`), so its bounds no longer hold and it is never
    /// skipped.
    pub(in crate::render) group_moved: Vec<std::sync::atomic::AtomicBool>,
    /// Each instance slot's group.
    pub(in crate::render) slot_group: Vec<u32>,
    /// Each group's solid's bounding sphere (its own space).
    pub(in crate::render) group_local: Vec<(Vec3, f32)>,
    /// Each instance slot's spheres as placed and as mirrored
    /// ([`instance_spheres`]), kept up to date as it moves: a moved
    /// group's instances are tested one by one against a view's planes.
    pub(in crate::render) slot_spheres: std::sync::RwLock<Vec<[(Vec3, f32); 2]>>,
    /// Each instance slot's position and cull radius as the vertex
    /// shader's scenery cull reads them (see [`instance_matrix`]).
    pub(in crate::render) instance_cull: std::sync::RwLock<Vec<[f32; 4]>>,
    /// The draws by section: each section's draw indices (ascending) and
    /// whether one is a road reflection receiver, so a view walks the
    /// draws of its visible sections only.
    pub(in crate::render) sections: Vec<(u32, Vec<u32>, bool)>,
    /// Per section (as `sections`): a sphere around its groups as built
    /// and as mirrored ([`WorldRenderer::mirrored_bounds`]), and whether
    /// it has a dome draw (never left out), so a view passes over whole
    /// sections outside it.
    pub(in crate::render) section_bounds: Vec<((Vec3, f32), (Vec3, f32), bool)>,
    /// Per section: a group of it has moved (`mark_moved`), so its bounds
    /// no longer hold.
    pub(in crate::render) section_moved: Vec<std::sync::atomic::AtomicBool>,
    /// Each group's section (its index in `sections`).
    pub(in crate::render) group_section: Vec<u32>,
    /// Per section: its groups.
    pub(in crate::render) section_groups: Vec<Vec<u32>>,
    /// Per group: its draws (ascending), and what they share — the dome
    /// or not, the flags the views cull by, the LOD bits and the
    /// instances.
    pub(in crate::render) group_draws: Vec<Vec<u32>>,
    pub(in crate::render) group_key: Vec<(bool, u16, u8, std::ops::Range<u32>)>,
    /// The draws (ascending) of the placements the game draws in the
    /// player's own view only, as overlays: MARKER_GPS_AID (0x5F3CC0 runs
    /// in World_DrawCars 0x6316E0 only for the view whose number is the
    /// player's, 0x631767; never in view 4 or the cube). Flagged in the
    /// SSR mask so traced reflections leave them out ([`overlay_draws`]).
    pub(in crate::render) overlay_draws: Vec<u32>,
    /// Headlight pools: `MAX_POOLS` quads of vertices, a fan's indices and
    /// one instance (the car's matrix) each, rewritten every frame.
    pub(in crate::render) pool_vertices: wgpu::Buffer,
    pub(in crate::render) pool_indices: wgpu::Buffer,
    pub(in crate::render) pool_instances: wgpu::Buffer,
    /// `2PLAYERHEADLIGHT1` (0x60C6B0 → 0x8A1C28): its bind-group slot and
    /// the pipeline its entry's blend and alpha test pick (0x5BCC40), if
    /// GLOBALB is loaded.
    pub(in crate::render) pool_texture: Option<(usize, PipelineKey)>,
    /// The road reflection's flares (0x5CAC9C, [`crate::glow::reflection_sprites`]):
    /// `MAX_REFLECTION_GLOWS` quads of vertices, rewritten every frame,
    /// their indices, one identity instance, and HEADLIGHTGLOW's slot and
    /// pipeline (its own blend), if GLOBALB is loaded.
    pub(in crate::render) reflection_glow_vertices: wgpu::Buffer,
    pub(in crate::render) reflection_glow_indices: wgpu::Buffer,
    pub(in crate::render) reflection_glow_instance: wgpu::Buffer,
    pub(in crate::render) reflection_glow_texture: Option<(usize, PipelineKey)>,
}
