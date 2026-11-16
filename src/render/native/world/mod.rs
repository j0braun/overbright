//! The world renderer (`WorldRenderer`, `WorldCore`) and what draws the
//! world: its build stages, views, passes and per-frame state.

pub mod cull;
pub mod draws;
pub mod prepared;
pub mod textures;
pub mod layouts;
pub mod pipelines;
pub mod env_cube;
pub mod placements;
pub mod plan;
pub mod record;
pub mod reflection;
pub mod shadows;
pub mod sprites;
pub mod rear_view;
mod build;
pub mod core;
#[cfg(test)]
mod reflection_tests;

use crate::render::*;

use self::build::buffers::{Buffers, Frames, Pipelines};
use self::build::draws::Draws;
use self::build::geometry::{Bounds, Geometry, Groups};
use self::build::textures::{SurfaceJobs, Textures, Uploaded};

/// The solid the game uses as the sky.
pub(in crate::render) const SKYDOME_SOLID: &str = "SKYDOME";
/// `Draw::lods` bits.
pub(in crate::render) const MAIN_LOD: u8 = 1;
pub(in crate::render) const LOW_LOD: u8 = 2;
/// The main view's LOD when g_WorldLodLevel is below 2 (the placement's
/// `mid_solid`, [`display::Display::world_lod_b`]).
pub(in crate::render) const MAIN_LOD_B: u8 = 4;

/// What to draw this frame.
pub struct FrameParams<'a> {
    pub camera: &'a Camera,
    pub aspect: f32,
    /// Sections to draw (the game's visibility list); `None` draws all.
    pub visible_sections: Option<&'a [u32]>,
    /// World Detail Extreme (clone deviation, docs/assets-distance.md):
    /// sections past `visible_sections` the main view draws with their
    /// lowest LOD (the reflection LOD, `low_solid`). None natively.
    pub far_sections: Option<&'a [u32]>,
    pub fog: Option<Fog>,
    /// Seconds since start, for animated textures.
    pub time: f32,
    /// Draw road reflections (view 4); `false` is
    /// `g_RoadReflectionEnable = 0`.
    pub road_reflections: bool,
    /// Local receiving road height for enhanced planar capture. None keeps the
    /// authored capture (frontend floors and callers without ground data).
    pub road_reflection_plane: Option<f32>,
    /// The dome's sky pair ([`crate::sky::TimeOfDay::sky`]).
    pub sky: SkyFrame,
    /// The race runs backward (0x89E7A4): picks which direction-only
    /// scenery the views cull (see [`WorldView::cull_flags`]).
    pub reversed: bool,
    /// The cars' headlight pools, for the road reflection. The first is
    /// the player's car (car 0), which also carries the headlight
    /// projector, view 9 ([`headlight_uniform`]).
    pub headlight_pools: &'a [HeadlightPool],
    /// The street lamps' flares as the road reflection draws them
    /// ([`crate::glow::world_reflection_sprites`]), with HEADLIGHTGLOW.
    pub reflection_glows: &'a [crate::particles::Quad],
    /// The Customize Display values the world draw reads.
    pub display: display::Display,
    /// The frame's settings in force (`FrameSettings::opts`).
    pub opts: &'a crate::render::RenderOptions,
    /// View 3, the rear-view mirror's camera ([`crate::rear_view::camera`]), when
    /// the frame draws it (screen layout 0x8707DC = 2, view 3 +8 set).
    pub rear_view: Option<&'a Camera>,
    /// The cars as boxes for Contact Shadows and Lamp Shadows (clone
    /// deviations, docs/lamps-shadows.md), the player's first; empty
    /// leaves them out.
    pub occluders: &'a [lamps::Occluder],
}

pub struct WorldRenderer {
    pub(in crate::render) core: std::sync::Arc<WorldCore>,
    /// Animated Water ([`WorldRenderer::set_hide_water`]).
    pub(in crate::render) hide_water: bool,
    /// Ambient Detail (docs/ambient-detail.md): made when the row is
    /// first on in game ([`WorldRenderer::ensure_ambient`]).
    pub(in crate::render) ambient: Option<Box<ambient_detail::AmbientDetail>>,
    /// `ambient` has not been drawn since it was made by
    /// [`WorldRenderer::warm_drive`] (a load's warm-up or the menus'
    /// preload) or since a new drive took this renderer with the props of
    /// the last one ([`WorldRenderer::new_drive_ambient`]): the next warm-up
    /// and the next [`WorldRenderer::ensure_ambient`] check it against the
    /// world as it is then (`AmbientDetail::made_for`) and make the props
    /// again if the world's placements that hold their surfaces changed, so
    /// they are always the first frame's.
    pub(in crate::render) ambient_check: bool,
    /// The weather for Wet Roads ([`WorldRenderer::set_weather`]): the
    /// rain's amount and the wetness; none (front-end scenes, never given
    /// one) keeps Wet Roads and the SSR mask off.
    /// Wet Roads' weather ([`WorldRenderer::set_weather`]).
    pub(in crate::render) weather: Option<(f32, f32, f32)>,
    /// Weather effects (docs/weather.md): made when a weather row is
    /// first on; the main view's camera they draw with this frame.
    pub(in crate::render) weather_fx: Option<weather_fx::WeatherFx>,
    pub(in crate::render) fx_view: Option<weather_fx::View>,
    /// The weather effects' CPU half, stepped on the tick
    /// ([`WorldRenderer::enh_tick`]); the draw reads it.
    pub(in crate::render) fx_sim: crate::render::enh::weather::host::WeatherSim,
    /// The ticks whose sky panorama rows the next frame commits.
    pub(in crate::render) sky_ticks: Vec<sky_ng::SkyTick>,
    /// Wet Road Interaction (docs/wet-road-car.md): made when the row
    /// is first on in a wet scene; bindings 12–14's placeholders.
    pub(in crate::render) wet_road: Option<wet_road::WetRoad>,
    /// Wet Road Interaction's volumetric spray (`wet_spray`).
    pub(in crate::render) spray_volume: Option<wet_spray::SprayVolume>,
    pub(in crate::render) wet_placeholder: wet_road::WetBindings,
    /// The main view's fog (value, colour) this frame, for the SSR mask.
    pub(in crate::render) main_fog: ([f32; 3], [f32; 3]),
    /// Screen-Space Reflections' mask target ([`WetMask`]), made on first
    /// use, its two pipelines (without and with the alpha test), and
    /// whether this frame's world drew it.
    pub(in crate::render) wet_mask: Option<WetMask>,
    /// By (Reversed-Z, Surface Detail: the lamps' module).
    pub(in crate::render) wet_mask_pipelines: Option<((bool, bool), [wgpu::RenderPipeline; 3])>,
    /// The mask from the main view's pipelines, by (scene mode, scene format, LOD Fade, Reversed-Z, Surface
    /// Detail) ([`WorldRenderer::mask_mrt_for`]).
    pub(in crate::render) mask_mrt_pipelines:
        Option<((SceneMode, wgpu::TextureFormat, bool, bool, bool), plan::MaskMrt)>,
    /// Whether the device is a discrete GPU (the mask from the main view's device class), once asked.
    pub(in crate::render) mask_from_main_discrete: Option<bool>,
    pub(in crate::render) wet_mask_drawn: bool,
    /// FSR 2's animation mask ([`AnimMask`]), made on first use
    /// under FSR 2, dropped without it; its pipelines per depth direction.
    pub(in crate::render) anim_mask: Option<AnimMask>,
    pub(in crate::render) anim_mask_pipelines: Option<((bool, bool), [wgpu::RenderPipeline; 6])>,
    pub(in crate::render) anim_mask_drawn: bool,
    /// The time (s) of the last frame the mask was drawn for.
    pub(in crate::render) anim_mask_time: Option<f32>,
    /// The scene's size the views are drawn at ([`WorldRenderer::set_view_size`];
    /// (0, 0) unknown): what the Graphics page's Road Reflection Res sizes
    /// view 4 from.
    pub(in crate::render) view_size: (u32, u32),
    /// The output's size ([`WorldRenderer::set_screen_size`]): Modern + RT's
    /// traced shadows follow it when the scene is supersampled.
    pub(in crate::render) screen_size: (u32, u32),
    /// Modern + RT's half-resolution traced shadows (made on first use).
    pub(in crate::render) rt_shadow: Option<rt_shadow::RtShadow>,
    /// Basis used by retained cube faces; reset when storage changes.
    pub(in crate::render) environment_enhanced: Option<bool>,
    /// The newest face-schedule index whose faces are in the cube
    /// (a GPU cache's bookkeeping, written by the draw;
    /// [`crate::render::display::env_faces_due`]). None: a new, remade
    /// or cleared cube.
    pub env_tick: Option<u64>,
    /// The main view's `Draw::lods` bit (MAIN_LOD or MAIN_LOD_B).
    pub(in crate::render) main_lod: u8,
    /// Instance slots of the SKYDOME, moved to the camera every frame.
    pub(in crate::render) sky_instances: Vec<u32>,
    pub(in crate::render) reflection_layout: wgpu::BindGroupLayout,
    pub(in crate::render) reflection_sampler: wgpu::Sampler,
    pub(in crate::render) reflection: Option<ReflectionTarget>,
    /// View 3's target, made on its first use.
    pub(in crate::render) rear: Option<RearViewTarget>,
    /// `RVM` and `RVM_MASK` as the world has them, for view 3's frame
    /// passes (built with the target).
    pub(in crate::render) rear_textures: (Option<Texture>, Option<Texture>),
    pub(in crate::render) rear_frame_passes: Option<rear_view::RearViewFrame>,
    /// Each placement's slots in `instances` (one per LOD drawn), for
    /// moving it (smackables).
    pub(in crate::render) placement_slots: Vec<Vec<u32>>,
    /// Each placement's cull radius, kept for `move_placement`.
    pub(in crate::render) placement_radius: Vec<f32>,
    /// Each placement's flags have [`HIDDEN_FLAG`] (see
    /// [`WorldRenderer::update_placement`]).
    pub(in crate::render) placement_hidden: Vec<bool>,
    /// Per placement, the transform [`WorldRenderer::move_placements`] last
    /// wrote to its slots (bits): a prop at rest, moved to the same place
    /// again, leaves its slots as they are. Dropped for a placement
    /// whose slots something else writes (`update_placement`), and all of
    /// them by `reset_placements`.
    pub(in crate::render) moved: std::sync::Mutex<crate::fxhash::FxHashMap<usize, [u32; 16]>>,
    /// How many pools the current frame holds.
    pub(in crate::render) pool_count: u32,
    /// How many reflection flare quads the current frame holds.
    pub(in crate::render) reflection_glow_count: u32,
    /// The emitter descriptions' textures (0x4901D0 on each +0xC): slot
    // prov: inferred render.road_plane_matrix
    /// and pipeline, the texture's own blend (**inferred**: 0x5C7E90 /
    /// 0x5C0E20 set the states from the texture as 0x5BCC40 does).
    pub(in crate::render) particle_textures: HashMap<u32, (usize, PipelineKey)>,
    pub(in crate::render) particle_vertices: wgpu::Buffer,
    pub(in crate::render) particle_indices: wgpu::Buffer,
    pub(in crate::render) particle_instance: wgpu::Buffer,
    /// This frame's runs: (texture, first quad, quads).
    pub(in crate::render) particle_runs: Vec<(u32, u32, u32)>,
    /// The flare textures (`crate::glow::TEXTURES`) by hash.
    pub(in crate::render) glow_textures: HashMap<u32, (usize, PipelineKey)>,
    /// worldnodepth with COLORWRITEENABLE 7 (0x5CBC00): into the back
    /// buffer, and into the light-streak buffer (8-bit RGBA).
    /// What [`WorldRenderer::set_samples`] rebuilds the scene's pipelines
    /// from: the scene's format, the texture arrays' use, the glow keys.
    pub(in crate::render) scene_format: wgpu::TextureFormat,
    pub(in crate::render) arrays: bool,
    pub(in crate::render) glow_keys: Vec<PipelineKey>,
    pub(in crate::render) glow_vertices: wgpu::Buffer,
    pub(in crate::render) glow_runs: Vec<(u32, u32, u32)>,
    /// The world texture layout and sampler, for textures added later
    /// (the cars' shadows).
    pub(in crate::render) texture_layout: wgpu::BindGroupLayout,
    pub(in crate::render) texture_sampler: wgpu::Sampler,
    /// docs/pack-hot-reload.md: the pack changes seen, and each slot's
    /// texture hash ([`WorldRenderer::sync_texture_pack`]).
    pub(in crate::render) pack_seen: u64,
    pub(in crate::render) pack_slots: Vec<Option<u32>>,
    /// The car shadow textures registered so far (`add_shadow_texture`):
    /// slot and pipeline.
    pub(in crate::render) shadow_textures: HashMap<u32, (usize, PipelineKey)>,
    /// Per shadow texture: uploaded as the Ultra tier's rebuilt underglow
    /// (`crate::neon_quality`), so a tier change uploads it again.
    pub(in crate::render) shadow_enhanced: HashMap<u32, bool>,
    pub(in crate::render) shadow_vertices: wgpu::Buffer,
    /// Textures added after the build (the cars' shadows), slots after
    /// the core's: per slot its group 1, scroll and array layer.
    pub(in crate::render) extra_textures: Vec<(wgpu::BindGroup, [f32; 8], u32)>,
    /// LOD Fade's per-instance state (clone deviation), made when the
    /// option (or the pop statistics) first asks for it.
    pub(in crate::render) fade: Option<FadeState>,
    /// Dynamic Lamps / Lamp Shadows / Contact Shadows (clone deviations):
    /// the light packs' lamps, and the GPU state made on first use.
    pub(in crate::render) lamps: Vec<lamps::Lamp>,
    pub(in crate::render) lamp: Option<lamps::LampState>,
    /// Surface Detail's normal-map candidates (slot, texture, class,
    /// worldreflect) and, once Materials + Normals is on, the maps
    /// ([`surface::SurfaceMaps`]).
    pub(in crate::render) surface_jobs: Vec<(usize, Texture, crate::surface_maps::Class, bool)>,
    pub(in crate::render) surface_maps: Option<surface::SurfaceMaps>,
    /// Light Probes (docs/lighting2.md): the bake target and field, made on first use.
    pub(in crate::render) probes: Option<probes::ProbeBaker>,
    /// The game folder and region the world came from (the probes' routes).
    pub(in crate::render) probe_source: Option<(std::path::PathBuf, String)>,
    /// Group 0's layout and HEADLIGHTTEX, for the lamps' frame groups.
    pub(in crate::render) frame_layout: wgpu::BindGroupLayout,
    pub(in crate::render) headlight: (wgpu::TextureView, wgpu::Sampler),
}

impl std::ops::Deref for WorldRenderer {
    type Target = WorldCore;
    fn deref(&self) -> &WorldCore {
        &self.core
    }
}

impl WorldRenderer {
    /// `opts`: the settings in force at the build (the texture filter, the
    /// cars' cube size).
    pub fn new(gpu: &Gpu, world: &World, color_format: wgpu::TextureFormat, opts: &crate::render::RenderOptions) -> WorldRenderer {
        let _t = crate::prof::span("WorldRenderer::new");
        let device = &gpu.device;

        // The build stages (`build`), in the order they always ran.
        let Geometry { bc, max_layers, cached, solid_base, vertex_count, geometry } = build::geometry::prepare_geometry(device, world);
        let Groups { groups, mut placement_slots, mut instance_data } = build::geometry::build_groups(world);

        let t_textures = crate::prof::span("WorldRenderer::new: draws and textures");
        let Textures { sampler, texture_layout, mut slots, animations, animation_of, default_texture, pool_texture, reflection_glow_texture, particle_textures, glow_textures, sky_table } =
            build::textures::assign_textures(device, world, opts);
        let Draws { draws, sky_instances, only_texture, group_bounds, mirrored_bounds, slot_group, group_local, slot_spheres } =
            build::draws::build_draws(world, &groups, &solid_base, &mut placement_slots, &mut instance_data, &mut slots, &animation_of, default_texture);
        let SurfaceJobs { surface_slots, surface_jobs } = build::textures::classify_surfaces(&slots.sources, &draws);
        let Uploaded { pack_slots, texture_scrolls, texture_arrays, texture_bind_groups, sky_groups } = build::textures::upload_textures(
            gpu,
            world,
            &slots.sources,
            &cached,
            bc,
            max_layers,
            &only_texture,
            &geometry,
            &sampler,
            &texture_layout,
            &sky_table,
            vertex_count,
            &instance_data,
            &draws,
            &animations,
        );
        drop(t_textures);
        let t_buffers = crate::prof::span("WorldRenderer::new: buffers, uniforms");
        let Buffers {
            vertices,
            indices,
            instances,
            pool_vertices,
            pool_indices,
            pool_instances,
            particle_vertices,
            glow_vertices,
            reflection_glow_vertices,
            reflection_glow_indices,
            reflection_glow_instance,
            particle_indices,
            shadow_vertices,
            particle_instance,
        } = build::buffers::make_buffers(device, geometry, &instance_data);
        let Frames { frame_layout, headlight_view, headlight_sampler, main_frame, reflection_frame, reflection_effect_frame, rear_frame, env_frames, env, env_frames_all } =
            build::buffers::make_frames(gpu, world, default_texture, bc, opts);
        let (reflection_layout, reflection_sampler, no_reflection) = build::buffers::make_reflection_binding(gpu);
        drop(t_buffers);
        let Pipelines { arrays, glow_keys, pipelines, reflection_pipelines, glow_pipelines, glow_streak_pipelines, env_multiview } =
            build::buffers::make_pipelines(gpu, color_format, &texture_arrays, &glow_textures, env_frames_all, &env);
        let Bounds { sections, group_section, group_draws, group_key, section_groups, section_bounds } = build::geometry::build_bounds(&draws, &group_bounds, &mirrored_bounds);
        let core = WorldCore {
            sky_rows: sky_ng::rows_of(&sky_ng::Palette::from_world(world)),
            sky_ng: std::sync::OnceLock::new(),
            sky_on: std::sync::atomic::AtomicBool::new(false),
            scene: std::sync::RwLock::new(std::sync::Arc::new(ScenePipelines { mode: SceneMode::NATIVE, pipelines, glow: glow_pipelines, glow_streak: glow_streak_pipelines, fade: None, car_shadow: HashMap::new(), depth: None, characters: HashMap::new() })),
            reflection_pipelines,
            main_frame,
            lamp_main: std::sync::RwLock::new(None),
            ambient_draw: std::sync::Mutex::new(None),
            lamp_main_rt: std::sync::RwLock::new(None),
            reflection_frame,
            reflection_effect_frame,
            rear_frame,
            env_frames,
            env,
            env_multiview,
            env_black: std::sync::atomic::AtomicBool::new(false),
            texture_bind_groups,
            texture_scrolls,
            texture_arrays,
            surface_slots,
            sky_groups,
            animations,
            sky_table,
            no_reflection,
            vertices,
            indices,
            instances,
            section_moved: sections.iter().map(|_| Default::default()).collect(),
            sections,
            section_bounds,
            group_section,
            section_groups,
            overlay_draws: overlay_draws(world.gps_aid.iter().copied(), &placement_slots, &slot_group, &group_draws),
            group_draws,
            group_key,
            draws,
            group_moved: group_bounds.iter().map(|_| Default::default()).collect(),
            instance_cull: std::sync::RwLock::new(instance_data.iter().map(instance_cull).collect()),
            mirrored_bounds,
            group_bounds,
            slot_group,
            group_local,
            slot_spheres: std::sync::RwLock::new(slot_spheres),
            pool_vertices,
            pool_indices,
            pool_instances,
            pool_texture,
            reflection_glow_vertices,
            reflection_glow_indices,
            reflection_glow_instance,
            reflection_glow_texture,
        };
        WorldRenderer {
            core: std::sync::Arc::new(core),
            hide_water: false,
            ambient: None,
            ambient_check: false,
            weather: None,
            weather_fx: None,
            fx_view: None,
            fx_sim: Default::default(),
            sky_ticks: Vec::new(),
            wet_road: None,
            spray_volume: None,
            wet_placeholder: wet_road::WetBindings::placeholder(device),
            main_fog: (NO_FOG.value, NO_FOG.color),
            wet_mask: None,
            wet_mask_pipelines: None,
            mask_mrt_pipelines: None,
            mask_from_main_discrete: None,
            wet_mask_drawn: false,
            anim_mask: None,
            anim_mask_pipelines: None,
            anim_mask_drawn: false,
            anim_mask_time: None,
            view_size: (0, 0),
            screen_size: (0, 0),
            rt_shadow: None,
            environment_enhanced: None,
            env_tick: None,
            main_lod: MAIN_LOD,
            sky_instances,
            reflection_layout,
            reflection_sampler,
            reflection: None,
            rear: None,
            rear_textures: (world.textures.get(&rear_view::RVM).cloned(), world.textures.get(&rear_view::RVM_MASK).cloned()),
            rear_frame_passes: None,
            placement_slots,
            placement_radius: world.placements.iter().map(|p| p.cull_radius).collect(),
            placement_hidden: world.placements.iter().map(|p| p.flags & HIDDEN_FLAG != 0).collect(),
            moved: Default::default(),
            pool_count: 0,
            reflection_glow_count: 0,
            particle_textures,
            particle_vertices,
            particle_indices,
            particle_instance,
            particle_runs: Vec::new(),
            glow_textures,
            scene_format: color_format,
            arrays,
            glow_keys,
            glow_vertices,
            glow_runs: Vec::new(),
            texture_layout,
            texture_sampler: sampler,
            pack_seen: pack_reload::generation(),
            pack_slots,
            shadow_textures: HashMap::new(),
            shadow_enhanced: HashMap::new(),
            shadow_vertices,
            extra_textures: Vec::new(),
            fade: None,
            lamps: { modern_light_bold::load_emitters(world); lamps::lamps_of(&world.light_packs) },
            lamp: None,
            surface_jobs: if arrays { surface_jobs } else { Vec::new() },
            surface_maps: None,
            probes: None,
            probe_source: world.cache.as_ref().and_then(|c| c.region()).map(|(r, n)| (r.to_path_buf(), n.to_string())),
            frame_layout,
            headlight: (headlight_view, headlight_sampler),
        }
    }
}

/// Convenience for tools: a camera looking at the middle of the world.
pub fn overview_camera(world: &World) -> Camera {
    let (lo, hi) = world.bounds();
    let center = (lo + hi) * 0.5;
    Camera::looking_at(Vec3::new(center.x, center.y - 600.0, 400.0), Vec3::new(center.x, center.y, 0.0))
}
