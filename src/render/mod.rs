//! A wgpu renderer for a loaded [`World`].
//!
//! All solids live in one vertex buffer and one index buffer. Every placement
//! becomes a per-instance matrix, grouped by (solid, section), so each
//! (solid, material) pair is a single instanced draw call no matter how often
//! the prop repeats, and whole sections can be switched off.
//!
//! Which effect a draw uses follows SPEED2.EXE:
//!
//! * per mesh, from its header flags (0x5C58B0): 0x40 → worldreflect;
//! * per draw (0x5C5940): an additive texture (+0x56 == 2) on the plain world
//!   effect switches to worldnofog; the SKYDOME switches to skymorph only
//!   while the time of day is fading ([`crate::sky`]); in the main view,
//!   instances the headlight projector (view 9) takes switch world and
//!   worldreflect to worldheadlight / worldheadlightreflect, which the
//!   shader applies per instance ([`headlight_uniform`]).
//!
//! Each view drops scenery instances that come out too small on screen
//! (0x5D6580 → 0x5D64E0, [`cull_scale`]); the vertex shader does that per
//! instance.
//!
//! The SKYDOME is not drawn as scenery (0x5DFAC6 skips its hash when
//! binding scenery models); 0x60F890 draws it on its own, centred on the
//! camera, in the main views only.
//!
//! A view records only the draws that can put a pixel on screen: whole
//! sections and instance groups outside its planes, and groups whose every
//! instance the scenery cull drops, are left out on the CPU ([`PassCull`]),
//! which leaves the image as it was. The game's draws of a frame (its world
//! views, the cube faces) can be recorded and encoded on render threads
//! ([`WorldRenderer::render_async`]) from the renderer's fixed part,
//! [`WorldCore`].
//!
//! Road reflections are a second pass (view 4): the scene from the main
//! camera with every prop flipped about its own origin, at the props' low
//! LODs, drawn into an off-screen target that worldreflect samples in
//! screen space. The cars' headlight pools are drawn into that image only
//! ([`HeadlightPool`]).

pub mod debug_env;
pub mod native;
pub use self::native::frame_settings::{FrameSettings, RenderOptions};
pub use self::core::gpu::*;
pub use self::core::jobs::*;
pub(crate) use self::core::pipeline_cache::*;
pub use self::core::target_pool as pool;
pub use self::core::upload::*;
pub use self::native::world::cull::*;
pub use self::native::world::draws::*;
pub use self::native::world::prepared;
pub(crate) use self::native::world::textures::*;
pub use native::world::pipelines::{HDR_FORMAT, REFLECTION_FORMAT, SceneMode, lamp_features};
pub(crate) use native::world::pipelines::{DepthKey, DepthPrepass, ENV_VIEWS, LF_ML_CHAR, ScenePipelines, WorldShared, car_shadow_keys, depth_prepass, depth_prepass_env, in_depth_prepass, make_anim_mask_pipeline, make_wet_mask_hidden_pipeline, make_wet_mask_pipeline, mask_road_depth_layout, overlay_draws, wet_uniform};
#[cfg_attr(not(test), allow(unused_imports))]
pub(crate) use native::world::pipelines::{LF_LAMPS, LF_ML_LOW, LF_SHADOW_Q_SHIFT, LF_SHADOWS, WET_MIN, depth_prepass_keys, make_pipeline};
pub(crate) use native::world::layouts::{array_layout, bind_texture, bind_world_texture, frame_layout, frame_layout_with, make_texture_layout, make_texture_layout_plain};
#[cfg_attr(not(test), allow(unused_imports))]
pub(crate) use self::core::shader::registry::{subgroup_clusters, world_module_source, world_module_source_for, world_module_source_modern, world_source, world_source_modern};
pub mod debug;
pub mod enh;
pub use self::debug::pop_meter::*;
use self::enh::geometry::lod_fade::*;
use self::native::world::plan::*;
pub use self::native::world::shadows::*;
pub use self::native::car;
pub use self::native::car::lighting;
pub use self::native::car::shader_variants as car_shader_variants;
pub use self::native::characters;
#[allow(unused_imports)]
pub(crate) use self::enh::materials::car::glass as car_glass;
pub use self::enh::post::blur_dof;
pub use self::enh::post::enhance;
pub use self::enh::post::fsr;
pub use self::enh::post::fsr2;
pub use self::enh::post::hdr_output;
pub use self::enh::post::photo_grade;
pub use self::enh::post::smaa;
pub use self::enh::post::ssao;
pub use self::enh::post::ssr;
pub use self::enh::post::temporal;
pub use self::enh::volumes::water;
pub use self::enh::volumes::water::hq as water_hq;
pub use self::enh::volumes::water::ng as water_ng;
pub use self::native::world::{FrameParams, WorldRenderer, overview_camera};
use self::native::world::{LOW_LOD, MAIN_LOD, MAIN_LOD_B, SKYDOME_SOLID};
pub use self::native::world::core::WorldCore;
pub use self::native::world::env_cube::EnvCube;
use self::native::world::env_cube::{ENV_FAR, ENV_SIZE, EnvMultiview, env_face_axes, env_multiview_depth};
use self::native::world::reflection::{REFLECTION_SIZE, ReflectionTarget, road_plane_matrix};
pub use self::native::world::rear_view::REAR_VIEW_SIZE;
use self::native::world::rear_view::RearViewTarget;
pub use self::native::world::plan::{AmbientWarm, AnimMotion, anim_values};
pub mod camera;
pub mod core;
pub mod display;
pub mod lamps;
pub mod modern_light;
pub mod modern_light_bold;
pub mod probes;
pub mod pop_meter;
pub mod postfx;
pub mod rain;
pub mod wet_car;
pub mod weather_fx;
pub mod wet_road;
pub mod wet_spray;
pub mod rear_view;
mod texture_pack;
pub mod pack_reload;
pub mod rt;
pub mod rt_shadow;
pub(crate) mod surface;
pub mod volumetric;
pub mod car_lights;
pub mod atmosphere;
pub mod sky_ng;
pub mod gpu_particles;
pub mod smoke_cinematic;
pub mod smoke_volume;
pub mod sparks_fire;
pub mod ambient_detail;
pub mod ambient_props;
pub mod volume_fx;
pub mod ui;

use std::collections::HashMap;

use anyhow::{Context, Result};
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use crate::fog::Fog;
use crate::sky::{SKY_TABLE, SkyFrame};
use crate::formats::hash::bin_hash;
use crate::formats::solid::{MESH_FLAG_REFLECTIVE, Vertex};
use crate::formats::texture::{Blend, PixelFormat, Scroll, Texture};
use crate::world::World;

pub use camera::Camera;

