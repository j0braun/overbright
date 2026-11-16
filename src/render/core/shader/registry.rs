//! Every composed shader module by a typed key: the embedded sources
//! (`include_str!`, one place) and the flags a key composes with.

use super::compose::{self, VariantFlag, VariantFlags};

/// A module's file table: (name, text), the root first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShaderId {
    /// The world module (world.wgsl and what it includes).
    World,
    /// The post chain (postfx.wgsl: native passes, High Precision Post,
    /// the glow's box, HDR Output's grade).
    PostFx,
    /// Rain FX (weather_fx.wgsl: common, rain, Next-Gen rain, haze).
    WeatherFx,
    /// VolumeFx (volume_fx.wgsl: common head and one file per section).
    VolumeFx,
    /// GPU smoke (gpu_particles.wgsl: common, compute, draw).
    GpuParticles,
    /// Cinematic smoke (smoke_cinematic.wgsl: common, compute, draw, full).
    SmokeCinematic,
    /// FSR 2 (fsr2.wgsl: the common part and one file per `//!pass`).
    Fsr2,
    /// The front end's quads (ui.wgsl; the colour-vision palette's part).
    Ui,
}

/// (name, text) of `src/render/{path}`, the name `path` (as an
/// `#include` writes it).
macro_rules! file {
    ($path:literal) => {
        ($path, include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/", $path)))
    };
}

/// (name, text) of `src/render/shaders/{path}` (the files cut from
/// the roots at the same positions; they move next to their Rust owners
/// later), the name `path`.
macro_rules! shader {
    ($path:literal) => {
        ($path, include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/shaders/", $path)))
    };
}

const WORLD_FILES: &[(&str, &str)] = &[
    ("world.wgsl", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/world.wgsl"))),
    ("world_prelude.wgsl", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/world_prelude.wgsl"))),
    ("wet_road_world.wgsl", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/wet_road_world.wgsl"))),
    shader!("native/world/common.wgsl"),
    shader!("native/world/bind_plain.wgsl"),
    shader!("native/world/bind_array.wgsl"),
    shader!("enh/lighting/lamps.wgsl"),
    shader!("enh/lighting/modern.wgsl"),
    shader!("enh/lighting/mlrt.wgsl"),
    shader!("enh/lighting/beams.wgsl"),
    shader!("enh/lighting/world_lit.wgsl"),
    shader!("enh/lighting/shadow_depth.wgsl"),
    shader!("native/world/vertex.wgsl"),
    shader!("enh/geometry/world_fade.wgsl"),
    shader!("native/world/fragment.wgsl"),
    shader!("enh/materials/world_reflect_enh.wgsl"),
    shader!("native/world/fragment_reflect.wgsl"),
    shader!("enh/weather/wetness.wgsl"),
    shader!("enh/materials/world_wet.wgsl"),
    shader!("enh/geometry/world_masks.wgsl"),
    shader!("native/world/fragment_sky.wgsl"),
    shader!("enh/geometry/world_coverage.wgsl"),
    shader!("enh/weather/rain_lit.wgsl"),
];

const POSTFX_FILES: &[(&str, &str)] = &[
    file!("postfx.wgsl"),
    shader!("native/post/post.wgsl"),
    shader!("enh/post/high_precision.wgsl"),
    shader!("enh/post/post_scale.wgsl"),
    shader!("enh/post/hdr_grade.wgsl"),
];

const WEATHER_FX_FILES: &[(&str, &str)] = &[
    file!("weather_fx.wgsl"),
    shader!("enh/weather/weather_common.wgsl"),
    shader!("enh/weather/rain.wgsl"),
    shader!("enh/weather/wetness.wgsl"),
    shader!("enh/weather/rain_ng.wgsl"),
    shader!("enh/weather/haze.wgsl"),
];

const VOLUME_FX_FILES: &[(&str, &str)] = &[
    file!("volume_fx.wgsl"),
    shader!("enh/volumes/volume_fx/volume_common.wgsl"),
    shader!("enh/volumes/volume_fx/pool.wgsl"),
    shader!("enh/volumes/volume_fx/splat.wgsl"),
    shader!("enh/volumes/volume_fx/resolve.wgsl"),
    shader!("enh/volumes/volume_fx/light.wgsl"),
    shader!("enh/volumes/volume_fx/march.wgsl"),
    shader!("enh/volumes/volume_fx/temporal.wgsl"),
    shader!("enh/volumes/volume_fx/composite.wgsl"),
];

const GPU_PARTICLES_FILES: &[(&str, &str)] = &[
    file!("gpu_particles.wgsl"),
    shader!("enh/volumes/smoke/gpu_particles_common.wgsl"),
    shader!("enh/volumes/smoke/gpu_particles_compute.wgsl"),
    shader!("enh/volumes/smoke/gpu_particles_draw.wgsl"),
];

const SMOKE_CINEMATIC_FILES: &[(&str, &str)] = &[
    file!("smoke_cinematic.wgsl"),
    shader!("enh/volumes/smoke/cinematic_common.wgsl"),
    shader!("enh/volumes/smoke/cinematic_compute.wgsl"),
    shader!("enh/volumes/smoke/cinematic_draw.wgsl"),
    shader!("enh/volumes/smoke/cinematic_full.wgsl"),
];

const FSR2_FILES: &[(&str, &str)] = &[
    file!("fsr2.wgsl"),
    shader!("enh/post/fsr2/common.wgsl"),
    shader!("enh/post/fsr2/motion.wgsl"),
    shader!("enh/post/fsr2/reconstruct.wgsl"),
    shader!("enh/post/fsr2/luma.wgsl"),
    shader!("enh/post/fsr2/depth_clip.wgsl"),
    shader!("enh/post/fsr2/snapshot.wgsl"),
    shader!("enh/post/fsr2/composition.wgsl"),
    shader!("enh/post/fsr2/lock.wgsl"),
    shader!("enh/post/fsr2/accumulate.wgsl"),
    shader!("enh/post/fsr2/output.wgsl"),
];

const UI_FILES: &[(&str, &str)] =
    &[file!("ui.wgsl"), shader!("enh/ui/palette.wgsl"), shader!("enh/ui/palette_fs.wgsl")];

impl ShaderId {
    pub const ALL: [ShaderId; 8] = [
        ShaderId::World,
        ShaderId::PostFx,
        ShaderId::WeatherFx,
        ShaderId::VolumeFx,
        ShaderId::GpuParticles,
        ShaderId::SmokeCinematic,
        ShaderId::Fsr2,
        ShaderId::Ui,
    ];

    /// The includes the root had before the split into files, left as `#include`
    /// lines by [`text`].
    pub fn kept(self) -> &'static [&'static str] {
        match self {
            ShaderId::World => &["world_prelude.wgsl", "wet_road_world.wgsl"],
            _ => &[],
        }
    }

    pub fn files(self) -> &'static [(&'static str, &'static str)] {
        match self {
            ShaderId::World => WORLD_FILES,
            ShaderId::PostFx => POSTFX_FILES,
            ShaderId::WeatherFx => WEATHER_FX_FILES,
            ShaderId::VolumeFx => VOLUME_FX_FILES,
            ShaderId::GpuParticles => GPU_PARTICLES_FILES,
            ShaderId::SmokeCinematic => SMOKE_CINEMATIC_FILES,
            ShaderId::Fsr2 => FSR2_FILES,
            ShaderId::Ui => UI_FILES,
        }
    }
}

/// `id`'s source as one file ([`compose::inline`]: the split's includes put
/// back, `#if` lines and the older includes kept), made once. Byte for byte the file it was cut
/// from (validate_tests.rs); for every module but the world (which
/// has `#if`s) it is also the module the GPU gets.
pub fn text(id: ShaderId) -> &'static str {
    static TEXTS: [std::sync::OnceLock<String>; ShaderId::ALL.len()] =
        [const { std::sync::OnceLock::new() }; ShaderId::ALL.len()];
    TEXTS[id as usize].get_or_init(|| compose::inline(id.files(), id.kept()).unwrap_or_else(|e| panic!("{id:?}: {e}")))
}

/// How a world draw's texture is bound (`TextureArrays`, or bound arrays).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextureBinding {
    Plain,
    Array,
    Bound,
}

impl TextureBinding {
    pub const ALL: [TextureBinding; 3] = [TextureBinding::Plain, TextureBinding::Array, TextureBinding::Bound];
}

/// One world module (`WorldShared::mode_modern`'s forms).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WorldKey {
    /// The lamps' module (clone deviations), else the native shaders.
    pub lamps: bool,
    /// Modern Lighting's tier (0 off, 1 Modern, 2 Modern + RT); lamps only.
    pub modern: u8,
    pub binding: TextureBinding,
    /// The environment cube's six faces in one pass.
    pub multiview: bool,
    /// The temporal pass's LOD bias (`MIP_BIAS`).
    pub biased: bool,
    /// The device's subgroup operations: the lamps' light-cluster loop in
    /// its subgroup form (`SUBGROUPS`; lamps only, [`subgroup_clusters`]).
    pub subgroups: bool,
}

/// Whether `device`'s world modules take the lamps' cluster loop in its
/// subgroup form: wgpu's `Features::SUBGROUP` on the device (asked for
/// where the adapter offers it, `Gpu::open`; Vulkan 1.1+ drivers with
/// subgroup operations in fragment shaders, DX12 at shader model 6.0 with
/// wave operations, so never through FXC). A property of the device: a
/// device's world modules are all one form, decided when the first is made.
pub fn subgroup_clusters(device: &wgpu::Device) -> bool {
    device.features().contains(wgpu::Features::SUBGROUP)
}

impl WorldKey {
    /// The key as the module is built: Modern only with the lamps, the
    /// tier at most 2; the subgroup form only with the lamps.
    pub fn normalized(self) -> WorldKey {
        WorldKey {
            modern: if self.lamps { self.modern.min(2) } else { 0 },
            subgroups: self.lamps && self.subgroups,
            ..self
        }
    }

    /// Every distinct world module (normalized keys).
    pub fn all() -> Vec<WorldKey> {
        let mut keys = Vec::new();
        for lamps in [false, true] {
            for modern in 0..=2u8 {
                for binding in TextureBinding::ALL {
                    for multiview in [false, true] {
                        for biased in [false, true] {
                            for subgroups in [false, true] {
                                let k = WorldKey { lamps, modern, binding, multiview, biased, subgroups };
                                if k.normalized() == k {
                                    keys.push(k);
                                }
                            }
                        }
                    }
                }
            }
        }
        keys
    }

    /// The flags the module composes with.
    pub fn flags(self) -> VariantFlags {
        let k = self.normalized();
        VariantFlags::NONE
            .with(VariantFlag::Lamps, k.lamps)
            .with(VariantFlag::Modern, k.modern >= 1)
            .with(VariantFlag::Mlrt, k.modern >= 2)
            // The lamps' module's clip position is invariant.
            .with(VariantFlag::InvariantPosition, k.lamps)
            .with(VariantFlag::TextureBindingPlain, k.binding == TextureBinding::Plain)
            .with(VariantFlag::TextureBindingArray, k.binding == TextureBinding::Array)
            .with(VariantFlag::TextureBindingBound, k.binding == TextureBinding::Bound)
            .with(VariantFlag::Multiview, k.multiview)
            .with(VariantFlag::MipBias, k.biased)
            .with(VariantFlag::Subgroups, k.subgroups)
    }
}

/// `id`'s files composed with `flags`.
pub fn compose(id: ShaderId, flags: VariantFlags) -> Result<String, compose::ComposeError> {
    compose::compose(id.files(), flags)
}

/// The world module for `key`. The sources are embedded and every key is
/// composed by the validation tests, so an error here is a source bug.
pub fn world(key: WorldKey) -> String {
    compose(ShaderId::World, key.flags()).unwrap_or_else(|e| panic!("world module {key:?}: {e}"))
}

/// world.wgsl's named section `name` (`// BEGIN {name}` … `// END
/// {name}`; the RTVIS blocks other modules take as they are).
pub fn world_section(name: &str) -> &'static str {
    compose::section(WORLD_FILES[0].0, text(ShaderId::World), name).unwrap_or_else(|e| panic!("{e}"))
}


/// world.wgsl for the cube in one pass: group 0 binding 0 holds the six
/// faces' frame values, and every entry point starts by taking its view's
/// (`frame` becomes a private copy; the shaders read the same values).
/// world.wgsl with its `// BEGIN LAMPS` … `// END LAMPS` lines (the
/// lamps' module) or without them (the native shaders).
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn world_source(lamps: bool) -> String {
    world_source_modern(
        lamps,
        u8::from(lamps && crate::render::modern_light::enabled(&crate::render::RenderOptions::in_force())),
    )
}

/// [`world_source`] with Modern Lighting's tier `modern` (0 off, 1
/// Modern, 2 Modern + RT; only with `lamps`).
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn world_source_modern(lamps: bool, modern: u8) -> String {
    world_module_source_modern(lamps, modern, false, false, false, false)
}

/// The world module's source for [`WorldShared::mode_with`]: world.wgsl
/// with or without the lamps, its texture block for texture arrays (bound
/// arrays with `bound`), the multiview and mip-bias forms.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn world_module_source(lamps: bool, arrays: bool, multiview: bool, biased: bool, bound: bool) -> String {
    world_module_source_modern(lamps, 0, arrays, multiview, biased, bound)
}

/// [`world_module_source`] with Modern Lighting's tier, composed by
/// [`core::shader::registry::world`] (`bound` only with `arrays`); the
/// cluster loop's plain form (a device without subgroups).
pub(crate) fn world_module_source_modern(lamps: bool, modern: u8, arrays: bool, multiview: bool, biased: bool, bound: bool) -> String {
    world_module_source_for(lamps, modern, arrays, multiview, biased, bound, false)
}

/// [`world_module_source_modern`] with the cluster loop's subgroup form
/// when `subgroups` (and the lamps: [`subgroup_clusters`]).
pub(crate) fn world_module_source_for(
    lamps: bool,
    modern: u8,
    arrays: bool,
    multiview: bool,
    biased: bool,
    bound: bool,
    subgroups: bool,
) -> String {
    use crate::render::core::shader::registry::{TextureBinding, WorldKey};
    let binding = match (arrays, bound) {
        (false, _) => TextureBinding::Plain,
        (true, false) => TextureBinding::Array,
        (true, true) => TextureBinding::Bound,
    };
    let key = WorldKey { lamps, modern, binding, multiview, biased, subgroups };
    crate::render::core::shader::registry::world(key)
}
