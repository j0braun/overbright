//! The native world's scene modes, pipelines and the device-shared
//! pipeline caches (audit plan render §1: moved from render/mod.rs).

use std::collections::HashMap;

use crate::formats::solid::Vertex;
use crate::formats::texture::Blend;
use crate::render::{
    ANIM_MASK_FORMAT, DEPTH_FORMAT, Effect, PipelineKey, TextureImmediates, WET_MASK_FORMAT, WET_MASK_TINT_FORMAT,
    array_layout, bound_arrays, debug_env, frame_layout, make_texture_layout, modern_light, modern_light_bold,
    note_pipeline_at, subgroup_clusters, temporal, world_module_source_for,
};

/// How the scene's pipelines draw (the main view, shadows, particles,
/// glows, the cars, the horizon fog): its sample count, and two Graphics
/// page deviations — Reversed-Z (near at depth 1, far at 0, GREATEREQUAL
/// tests, cleared to 0) and alpha-to-coverage for the alpha-tested draws
/// (with the page's MSAA). [`SceneMode::NATIVE`] at one sample is the
/// native path. The road reflection, the mirror and the cube keep the
/// native depth (their own targets and pipelines). With `hdr` (the
/// Graphics page's High Precision Post, docs/hdr-post.md) the scene and
/// the streak buffer are [`HDR_FORMAT`] and the scene's pipelines draw
/// into that ([`SceneMode::target`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SceneMode {
    pub samples: u32,
    pub reversed: bool,
    pub coverage: bool,
    pub hdr: bool,
    /// Dynamic Lamps / Lamp Shadows / Contact Shadows (clone deviations,
    /// docs/lamps-shadows.md): the main view's pipelines from world.wgsl
    /// with its LAMPS blocks (else without them: the native shaders).
    pub lamps: bool,
    /// Modern Lighting (docs/modern-lighting.md): 0 off, 1 the lamps'
    /// module with its MODERN blocks, 2 also with the ray-query (MLRT)
    /// lines and the traced scene at group 0 binding 11 (Modern + RT).
    pub modern: u8,
    /// The temporal pass's texture LOD bias (docs/temporal.md), in
    /// quarter mip levels: 0 native (the native modules); else the main
    /// view's world and car modules sample with `textureSampleBias`.
    pub bias_q: i8,
    /// The lamps' module's feature set in force ([`lamp_features`]; 0
    /// without the lamps), its pipelines specialised to it
    /// ([`SceneMode::lamp_constants`]).
    pub lamp_features: u16,
}

/// Bits of [`SceneMode::lamp_features`]: an option on whose uniform
/// the lamps' module tests (bit clear: it is 0 in every frame).
pub(crate) const LF_LAMPS: u16 = 1;
const LF_CONTACT: u16 = 2;
pub(crate) const LF_SHADOWS: u16 = 4;
const LF_BEAMS: u16 = 8;
const LF_SURFACE: u16 = 16;
/// Bits 5–6: Lamp Shadows' quality (0 off, 1 Low, 2 High).
pub(crate) const LF_SHADOW_Q_SHIFT: u8 = 5;
/// Modern Lighting's Low tier (else Modern / Modern + RT, which world.wgsl
/// tells apart by the params' ray count only: `D9_ML_TIER` 1 or 2).
pub(crate) const LF_ML_LOW: u16 = 128;
/// The NIS characters' pipelines under Modern + RT (their rays in the
/// colour shader; every other draw reads the half-resolution pass).
pub(crate) const LF_ML_CHAR: u16 = 256;
/// The main view's road (`Effect::Reflect`) pipelines for a frame whose
/// road is dry (its wet.x 0): the Wet Roads branch compiled out
/// (`D9_WET` false), whose terms otherwise set the function's register
/// allocation for every road pixel ([`DepthPrepass::equal_dry`]).
pub(crate) const LF_DRY: u16 = 512;

/// The feature set of the options in force, as `lamps::plan_lamps`
/// sets the main view's uniforms from them: lamps.x (Dynamic Lamps or
/// Modern Lighting), lamps.y (Contact Shadows), lamps.z (Lamp Shadows'
/// row), cluster_near.z (Headlight Beams' count), cull.z (Surface Detail).
pub fn lamp_features(opts: &crate::render::RenderOptions) -> u16 {
    let q = u16::from(opts.lamp_shadows.min(2));
    let mut f = q << LF_SHADOW_Q_SHIFT;
    if opts.dynamic_lamps || modern_light::enabled(opts) {
        f |= LF_LAMPS;
    }
    if opts.contact_shadows {
        f |= LF_CONTACT;
    }
    if q > 0 {
        f |= LF_SHADOWS;
    }
    if opts.headlight_beams {
        f |= LF_BEAMS;
    }
    if opts.surface_detail > 0 {
        f |= LF_SURFACE;
    }
    if modern_light_bold::tier(opts) == modern_light_bold::Tier::Low {
        f |= LF_ML_LOW;
    }
    f
}

/// The scene's and the screen passes' format with High Precision Post
/// (clone deviation; native: the 8-bit back buffer's).
pub const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

impl SceneMode {
    pub const NATIVE: SceneMode = SceneMode {
        samples: 1,
        reversed: false,
        coverage: false,
        hdr: false,
        lamps: false,
        modern: 0,
        bias_q: 0,
        lamp_features: 0,
    };

    /// The colour format a pipeline of this mode draws into: `format`, or
    /// [`HDR_FORMAT`] with High Precision Post.
    pub fn target(self, format: wgpu::TextureFormat) -> wgpu::TextureFormat {
        if self.hdr { HDR_FORMAT } else { format }
    }

    /// The native depth and alpha test at `samples`.
    pub fn plain(samples: u32) -> SceneMode {
        SceneMode { samples, ..SceneMode::NATIVE }
    }

    /// The scene at `samples` with the Graphics page's values in force.
    pub fn current(opts: &crate::render::RenderOptions, samples: u32) -> SceneMode {
        let lamps = opts.dynamic_lamps
            || opts.lamp_shadows > 0
            || opts.contact_shadows
            || opts.headlight_beams
            || opts.surface_detail > 0
            || modern_light::enabled(opts);
        SceneMode {
            samples,
            reversed: opts.reversed_z,
            coverage: samples > 1 && opts.msaa_override().is_some(),
            hdr: opts.hdr_active,
            lamps,
            modern: modern_light_bold::scene_modern(opts),
            bias_q: if opts.temporal_frame { temporal::mip_bias_q(opts) } else { 0 },
            lamp_features: if lamps { lamp_features(opts) } else { 0 },
        }
    }

    /// [`SceneMode::current`] as a drive's frames will have it,
    /// asked at its load (the menus' frames leave the temporal pass off:
    /// `qol::set_temporal_frame(temporal() && driving && no garage)`).
    pub fn driving(opts: &crate::render::RenderOptions, samples: u32) -> SceneMode {
        let bias_q = if opts.temporal { temporal::mip_bias_q(opts) } else { 0 };
        SceneMode { bias_q, ..SceneMode::current(opts, samples) }
    }

    /// world.efx's ZFUNC LESSEQUAL, or GREATEREQUAL reversed.
    pub fn compare(self) -> wgpu::CompareFunction {
        if self.reversed { wgpu::CompareFunction::GreaterEqual } else { wgpu::CompareFunction::LessEqual }
    }

    /// The depth a pass clears to: the far plane's.
    pub fn clear_depth(self) -> f32 {
        if self.reversed { 0.0 } else { 1.0 }
    }

    /// The fragment shaders' `SAMPLES` override (alpha-to-coverage).
    pub fn constants(self) -> [(&'static str, f64); 1] {
        [("SAMPLES", f64::from(self.samples))]
    }

    /// The lamps' module's feature constants (world.wgsl `D9_*`; none
    /// without the lamps, or with `NFSU2_LAMP_SPECIALISE=0`: every branch
    /// kept, the uniforms deciding).
    pub fn lamp_constants(self) -> Vec<(&'static str, f64)> {
        static OFF: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if !self.lamps || *OFF.get_or_init(|| crate::render::debug_env::nfsu2_lamp_specialise_off()) {
            return Vec::new();
        }
        let f = self.lamp_features;
        let on = |bit: u16| f64::from(u8::from(f & bit != 0));
        let mut c = vec![
            ("D9_LAMPS", on(LF_LAMPS)),
            ("D9_CONTACT", on(LF_CONTACT)),
            ("D9_SHADOWS", on(LF_SHADOWS)),
            ("D9_SHADOW_Q", f64::from(f >> LF_SHADOW_Q_SHIFT & 3)),
            ("D9_BEAMS", on(LF_BEAMS)),
            ("D9_SURFACE", on(LF_SURFACE)),
        ];
        // Modern Lighting's (its MODERN lines only in these modules).
        if self.modern > 0 {
            c.push(("D9_ML_TIER", if self.lamp_features & LF_ML_LOW != 0 { 1.0 } else { 2.0 }));
            c.push(("D9_ML_DEBUG", f64::from(u8::from(modern_light::debug()))));
            // Modern + RT's module only: the characters' in-shader rays.
            if self.modern == 2 {
                c.push(("D9_ML_CHAR", on(LF_ML_CHAR)));
            }
        }
        // The dry road's set only (the others keep the default, true).
        if f & LF_DRY != 0 {
            c.push(("D9_WET", 0.0));
        }
        c
    }

    /// The fragment constants a pipeline of this mode sets: `SAMPLES` with
    /// alpha-to-coverage, `MIP_BIAS` in the biased modules.
    pub fn fragment_constants(self) -> Vec<(&'static str, f64)> {
        let mut c = if self.coverage { self.constants().to_vec() } else { Vec::new() };
        if self.bias_q != 0 {
            c.push(("MIP_BIAS", f64::from(self.bias_q) / 4.0));
        }
        c
    }
}
pub const REFLECTION_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// The world's pipelines into the scene and the streak buffer at one
/// sample count (1, or the anti-aliasing's).
pub(crate) struct ScenePipelines {
    pub(crate) mode: SceneMode,
    pub(crate) pipelines: HashMap<PipelineKey, wgpu::RenderPipeline>,
    pub(crate) glow: HashMap<PipelineKey, wgpu::RenderPipeline>,
    pub(crate) glow_streak: HashMap<PipelineKey, wgpu::RenderPipeline>,
    /// LOD Fade's main-view pipelines, once asked for
    /// ([`WorldRenderer::ensure_fade_pipelines`]): reading the fades only
    /// (for groups none of whose instances is part-way), and dithering.
    pub(crate) fade: Option<[HashMap<PipelineKey, wgpu::RenderPipeline>; 2]>,
    /// The shadow texture's pipelines with the lamps' module (empty
    /// otherwise), keyed as `Effect::CarShadow` + the texture's blend.
    pub(crate) car_shadow: HashMap<PipelineKey, wgpu::RenderPipeline>,
    /// Depth pre-pass (clone, enhanced lighting only); None: the
    /// single native pass.
    pub(crate) depth: Option<std::sync::Arc<DepthPrepass>>,
    /// The NIS characters' world pipelines under Modern + RT (their
    /// rays traced in the colour shader, `D9_ML_CHAR`); empty otherwise
    /// (they take `pipelines`).
    pub(crate) characters: HashMap<PipelineKey, wgpu::RenderPipeline>,
}

/// The main view's depth pre-pass pipelines (depth only, by
/// [`DepthKey`]) and the colour pipelines its draws then take: the same
/// as [`ScenePipelines::pipelines`] / `fade` (index 0 native, 1 the fades
/// read, 2 dithered) but testing EQUAL without writing depth, which keeps
/// early depth for the alpha-tested draws (their discard no longer has a
/// depth write to hold back).
pub(crate) struct DepthPrepass {
    pub(crate) depth: HashMap<DepthKey, wgpu::RenderPipeline>,
    pub(crate) equal: [HashMap<PipelineKey, wgpu::RenderPipeline>; 3],
    /// The road's (`Effect::Reflect`) of `equal`, for a frame whose road is
    /// dry (the lamps' module only; [`LF_DRY`]): the same draw, the same
    /// pixels (the branch they leave out is not taken on a dry road), in
    /// fewer registers. Empty without the lamps.
    pub(crate) equal_dry: [HashMap<PipelineKey, wgpu::RenderPipeline>; 3],
}

impl DepthPrepass {
    /// `mode`'s dry-road set of `equal[fade]` (empty without the lamps).
    pub(crate) fn dry_for(
        shared: &WorldShared,
        device: &wgpu::Device,
        arrays: bool,
        mode: SceneMode,
        format: wgpu::TextureFormat,
        fade: u8,
    ) -> HashMap<PipelineKey, wgpu::RenderPipeline> {
        if !mode.lamps {
            return HashMap::new();
        }
        let keys: Vec<PipelineKey> = depth_prepass_keys().into_iter().filter(|k| k.effect == Effect::Reflect).collect();
        let dry = SceneMode { lamp_features: mode.lamp_features | LF_DRY, ..mode };
        shared.equal_pipelines_of(device, arrays, dry, format, fade, &keys)
    }
}

/// The colour pipelines' keys a pre-pass draw can have.
pub(crate) fn depth_prepass_keys() -> Vec<PipelineKey> {
    PipelineKey::all().into_iter().filter(|&k| in_depth_prepass(k, false, false)).collect()
}

/// A depth pre-pass pipeline's form: the colour pipeline's vertex
/// entry (`vs_reflect` for worldreflect), its alpha test, and LOD Fade's
/// mode (0 none, 1 the fades read, 2 the screen-door dither).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct DepthKey {
    pub(crate) reflect: bool,
    pub(crate) alpha_test: bool,
    pub(crate) fade: u8,
}

impl DepthKey {
    fn all() -> Vec<DepthKey> {
        let mut keys = Vec::new();
        for reflect in [false, true] {
            for alpha_test in [false, true] {
                for fade in 0..3 {
                    keys.push(DepthKey { reflect, alpha_test, fade });
                }
            }
        }
        keys
    }
}

/// Whether the main view draws a depth pre-pass in `mode`: only with
/// the lamps' module (Dynamic Lamps, the shadows, Surface Detail, Modern
/// Lighting: the expensive per-pixel shading the pre-pass runs once per
/// pixel); never natively. `NFSU2_DEPTH_PREPASS=0` turns it off (A/B).
pub(crate) fn depth_prepass(mode: SceneMode) -> bool {
    mode.lamps && depth_prepass_env() != Some(0)
}

/// `NFSU2_DEPTH_PREPASS` (measurements): 0 off, 2 the pre-pass with the
/// colour pass's own (LESSEQUAL, depth-writing) pipelines.
pub(crate) fn depth_prepass_env() -> Option<u8> {
    static ENV: std::sync::OnceLock<Option<u8>> = std::sync::OnceLock::new();
    *ENV.get_or_init(|| debug_env::nfsu2_depth_prepass())
}

/// Whether `key` is drawn into the pre-pass: the opaque (depth
/// writing) draws but the sky; alpha-tested ones only without
/// alpha-to-coverage (whose sample mask the pre-pass does not repeat).
pub(crate) fn in_depth_prepass(key: PipelineKey, sky: bool, coverage: bool) -> bool {
    !sky && key.blend == Blend::None
        && matches!(key.effect, Effect::World | Effect::NoFog | Effect::Reflect)
        && !(key.alpha_test && coverage)
}

/// The `Effect::CarShadow` keys (alpha-blended shadow textures).
pub(crate) fn car_shadow_keys() -> Vec<PipelineKey> {
    [false, true].map(|alpha_test| PipelineKey { effect: Effect::CarShadow, blend: Blend::Alpha, alpha_test }).to_vec()
}

/// The cube's six faces as one multiview pass's views (all six layers,
/// `multiview_mask` 0b111111; Metal's `view_index` is only defined so).
pub(crate) const ENV_VIEWS: u32 = 0b11_1111;

/// What every world renderer of a device shares (as [`car::CarShared`]
/// does for the cars): the world shader in its two forms (texture arrays
/// or a bind group per texture), their pipeline layouts and every pipeline
/// built so far, by form, colour format, write mask and key. A front-end
/// platform or a region loaded after the first then builds no pipelines.
/// Bind group layouts with equal entries are one object on a wgpu device,
/// so the renderers' own groups fit these layouts.
pub(crate) struct WorldShared {
    /// Without and with texture arrays, each for one view or the cube's
    /// six at once (multiview, [`EnvMultiview`]).
    /// (and with world.wgsl's LAMPS blocks: the second four).
    modes: [std::sync::OnceLock<WorldMode>; 48],
    built:
        std::sync::Mutex<HashMap<(bool, bool, SceneMode, wgpu::TextureFormat, u32, PipelineKey), wgpu::RenderPipeline>>,
    /// LOD Fade's main-view pipelines ([`WorldShared::fade_pipelines`]).
    built_fade:
        std::sync::Mutex<HashMap<(bool, SceneMode, wgpu::TextureFormat, bool, PipelineKey), wgpu::RenderPipeline>>,
    /// The depth pre-pass pipelines ([`WorldShared::depth_pipelines`]).
    built_depth: std::sync::Mutex<HashMap<(bool, SceneMode, DepthKey), wgpu::RenderPipeline>>,
    /// The pre-pass's EQUAL colour pipelines ([`WorldShared::equal_pipelines`]).
    built_equal:
        std::sync::Mutex<HashMap<(bool, SceneMode, wgpu::TextureFormat, u8, PipelineKey), wgpu::RenderPipeline>>,
    /// Their worldreflect keys writing the SSR mask too ([`WorldShared::mrt_pipelines`]).
    built_mrt:
        std::sync::Mutex<HashMap<(bool, SceneMode, wgpu::TextureFormat, u8, PipelineKey), wgpu::RenderPipeline>>,
    /// The device's pipeline cache ([`DiskPipelineCache`]).
    cache: Option<wgpu::PipelineCache>,
    /// The device's [`crate::render::Gpu::build_tag`].
    tag: u64,
}

pub(crate) struct WorldMode {
    pub(crate) shader: wgpu::ShaderModule,
    pub(crate) layout: wgpu::PipelineLayout,
    /// skymorph.efx's, with SKYMORPHTEXTURE (group 3).
    pub(crate) morph_layout: wgpu::PipelineLayout,
    /// The SSR mask's hidden road ([`make_wet_mask_hidden_pipeline`]):
    /// the mask's own depth in group 3 ([`mask_road_depth_layout`]).
    pub(crate) mask_hidden_layout: wgpu::PipelineLayout,
}

/// Group 3 of the SSR mask's hidden road (world_masks.wgsl
/// `mask_road_depth`, binding 2): the mask's own depth, read.
pub(crate) fn mask_road_depth_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    crate::render::core::device_cache::uncached_bind_group_layout(device, &wgpu::BindGroupLayoutDescriptor {
        label: Some("ssr mask road depth"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 2,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Depth,
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        }],
    })
}

impl WorldShared {
    pub(crate) fn new(cache: Option<wgpu::PipelineCache>, tag: u64) -> WorldShared {
        WorldShared {
            modes: std::array::from_fn(|_| Default::default()),
            built: Default::default(),
            built_fade: Default::default(),
            built_depth: Default::default(),
            built_equal: Default::default(),
            built_mrt: Default::default(),
            cache,
            tag,
        }
    }

    #[cfg(test)]
    pub(crate) fn mode(&self, device: &wgpu::Device, arrays: bool, multiview: bool) -> &WorldMode {
        self.mode_with(device, arrays, multiview, false, false)
    }

    /// A world module and its layouts, with world.wgsl's LAMPS blocks when `lamps`
    /// (Dynamic Lamps and the shadows, clone deviations); without them the
    /// shaders are the native ones, untouched.
    pub(crate) fn mode_with(
        &self,
        device: &wgpu::Device,
        arrays: bool,
        multiview: bool,
        lamps: bool,
        biased: bool,
    ) -> &WorldMode {
        self.mode_modern(device, arrays, multiview, lamps, 0, biased)
    }

    /// [`WorldShared::mode_with`] with Modern Lighting's tier (0 off,
    /// 1 Modern, 2 Modern + RT: group 0 with the traced scene).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn mode_modern(
        &self,
        device: &wgpu::Device,
        arrays: bool,
        multiview: bool,
        lamps: bool,
        modern: u8,
        biased: bool,
    ) -> &WorldMode {
        let modern = if lamps { modern.min(2) } else { 0 };
        self.modes[usize::from(arrays)
            + 2 * usize::from(multiview)
            + 4 * usize::from(lamps)
            + 8 * usize::from(biased)
            + 16 * usize::from(modern)]
        .get_or_init(|| {
            // The lamps' cluster loop in its subgroup form where the device
            // has subgroup operations (the same sums; docs/lamps-shadows.md
            // "The cluster loop"), else the plain loop: one form per device.
            let source = world_module_source_for(
                lamps,
                modern,
                arrays,
                multiview,
                biased,
                arrays && bound_arrays(device),
                lamps && subgroup_clusters(device),
            );
            let shader = crate::render::core::device_cache::uncached_module(device, self.tag, wgpu::ShaderModuleDescriptor {
                label: Some("world"),
                source: wgpu::ShaderSource::Wgsl(source.into()),
            });
            let frame = if modern == 2 { modern_light_bold::frame_layout_rt(device) } else { frame_layout(device) };
            let (group1, morph_group, immediate_size) = if arrays {
                (
                    array_layout(device, bound_arrays(device)),
                    make_texture_layout(device, "skymorph texture", false),
                    std::mem::size_of::<TextureImmediates>() as u32,
                )
            } else {
                (make_texture_layout(device, "texture", true), make_texture_layout(device, "skymorph texture", true), 0)
            };
            let reflection = make_texture_layout(device, "reflection", false);
            let layout = crate::render::core::device_cache::uncached_pipeline_layout(device, &wgpu::PipelineLayoutDescriptor {
                label: Some("world"),
                bind_group_layouts: &[Some(&frame), Some(&group1), Some(&reflection)],
                immediate_size,
            });
            let morph_layout = crate::render::core::device_cache::uncached_pipeline_layout(device, &wgpu::PipelineLayoutDescriptor {
                label: Some("skymorph"),
                bind_group_layouts: &[Some(&frame), Some(&group1), Some(&reflection), Some(&morph_group)],
                immediate_size,
            });
            let road_depth = mask_road_depth_layout(device);
            let mask_hidden_layout =
                crate::render::core::device_cache::uncached_pipeline_layout(device, &wgpu::PipelineLayoutDescriptor {
                    label: Some("ssr mask hidden road"),
                    bind_group_layouts: &[Some(&frame), Some(&group1), Some(&reflection), Some(&road_depth)],
                    immediate_size,
                });
            WorldMode { shader, layout, morph_layout, mask_hidden_layout }
        })
    }

    /// The pipelines for `keys`, built now (side by side) where not yet.
    #[track_caller]
    pub(crate) fn pipelines(
        &self,
        device: &wgpu::Device,
        arrays: bool,
        format: wgpu::TextureFormat,
        mask: wgpu::ColorWrites,
        keys: &[PipelineKey],
    ) -> HashMap<PipelineKey, wgpu::RenderPipeline> {
        self.pipelines_for(device, arrays, false, SceneMode::NATIVE, format, mask, keys)
    }

    #[allow(clippy::too_many_arguments)]
    #[track_caller]
    pub(crate) fn pipelines_for(
        &self,
        device: &wgpu::Device,
        arrays: bool,
        multiview: bool,
        scene: SceneMode,
        format: wgpu::TextureFormat,
        mask: wgpu::ColorWrites,
        keys: &[PipelineKey],
    ) -> HashMap<PipelineKey, wgpu::RenderPipeline> {
        use rayon::prelude::*;
        let at = std::panic::Location::caller();
        let mode =
            self.mode_modern(device, arrays, multiview, scene.lamps, scene.modern, scene.bias_q != 0);
        let id = |k: PipelineKey| (arrays, multiview, scene, format, mask.bits(), k);
        let missing: Vec<PipelineKey> = {
            let built = self.built.lock().unwrap();
            let mut m: Vec<PipelineKey> = keys.iter().copied().filter(|&k| !built.contains_key(&id(k))).collect();
            m.sort();
            m.dedup();
            m
        };
        // The driver's compiles are most of the time: on all cores, outside
        // the lock (the device takes pipelines from any thread).
        let made: Vec<(PipelineKey, wgpu::RenderPipeline)> = missing
            .into_par_iter()
            .map(|k| {
                let layout = if k.effect == Effect::SkyMorph { &mode.morph_layout } else { &mode.layout };
                (k, {
                    note_pipeline_at(at, self.tag);
                    make_pipeline(
                        device,
                        layout,
                        &mode.shader,
                        k,
                        format,
                        mask,
                        multiview.then(|| std::num::NonZeroU32::new(ENV_VIEWS).unwrap()),
                        scene,
                        self.cache.as_ref(),
                        0,
                    )
                })
            })
            .collect();
        let mut built = self.built.lock().unwrap();
        for (k, p) in made {
            built.entry(id(k)).or_insert(p);
        }
        keys.iter().map(|&k| (k, built[&id(k)].clone())).collect()
    }
}

impl WorldShared {
    /// LOD Fade's main-view pipelines (clone deviation) for `keys`, built
    /// on first use: [`make_pipeline`] with `fade`.
    #[track_caller]
    pub(crate) fn fade_pipelines(
        &self,
        device: &wgpu::Device,
        arrays: bool,
        scene: SceneMode,
        format: wgpu::TextureFormat,
        keys: &[PipelineKey],
        dither: bool,
    ) -> HashMap<PipelineKey, wgpu::RenderPipeline> {
        use rayon::prelude::*;
        let at = std::panic::Location::caller();
        let mode =
            self.mode_modern(device, arrays, false, scene.lamps, scene.modern, scene.bias_q != 0);
        let id = |k: PipelineKey| (arrays, scene, format, dither, k);
        let missing: Vec<PipelineKey> = {
            let built = self.built_fade.lock().unwrap();
            keys.iter().copied().filter(|&k| !built.contains_key(&id(k))).collect()
        };
        let made: Vec<(PipelineKey, wgpu::RenderPipeline)> = missing
            .into_par_iter()
            .map(|k| {
                let layout = if k.effect == Effect::SkyMorph { &mode.morph_layout } else { &mode.layout };
                (k, {
                    note_pipeline_at(at, self.tag);
                    make_pipeline(
                        device,
                        layout,
                        &mode.shader,
                        k,
                        format,
                        wgpu::ColorWrites::ALL,
                        None,
                        scene,
                        self.cache.as_ref(),
                        1 + u8::from(dither),
                    )
                })
            })
            .collect();
        let mut built = self.built_fade.lock().unwrap();
        for (k, p) in made {
            built.entry(id(k)).or_insert(p);
        }
        keys.iter().map(|&k| (k, built[&id(k)].clone())).collect()
    }
}

impl WorldShared {
    /// The main view's depth pre-pass pipelines in `scene` (the lamps'
    /// module), built on first use.
    #[track_caller]
    pub(crate) fn depth_pipelines(
        &self,
        device: &wgpu::Device,
        arrays: bool,
        scene: SceneMode,
    ) -> HashMap<DepthKey, wgpu::RenderPipeline> {
        use rayon::prelude::*;
        let at = std::panic::Location::caller();
        let mode =
            self.mode_modern(device, arrays, false, scene.lamps, scene.modern, scene.bias_q != 0);
        let all = DepthKey::all();
        let missing: Vec<DepthKey> = {
            let built = self.built_depth.lock().unwrap();
            all.iter().copied().filter(|&k| !built.contains_key(&(arrays, scene, k))).collect()
        };
        let made: Vec<(DepthKey, wgpu::RenderPipeline)> = missing
            .into_par_iter()
            .map(|k| {
                (k, {
                    note_pipeline_at(at, self.tag);
                    make_depth_pipeline(device, &mode.layout, &mode.shader, k, scene, self.cache.as_ref())
                })
            })
            .collect();
        let mut built = self.built_depth.lock().unwrap();
        for (k, p) in made {
            built.entry((arrays, scene, k)).or_insert(p);
        }
        all.iter().map(|&k| (k, built[&(arrays, scene, k)].clone())).collect()
    }
}

impl WorldShared {
    /// The pre-pass draws' colour pipelines ([`DepthPrepass::equal`])
    /// with LOD Fade's mode `fade`, built on first use.
    #[track_caller]
    pub(crate) fn equal_pipelines(
        &self,
        device: &wgpu::Device,
        arrays: bool,
        scene: SceneMode,
        format: wgpu::TextureFormat,
        fade: u8,
    ) -> HashMap<PipelineKey, wgpu::RenderPipeline> {
        self.equal_pipelines_of(device, arrays, scene, format, fade, &depth_prepass_keys())
    }

    /// [`WorldShared::equal_pipelines`] for `keys` only.
    #[track_caller]
    pub(crate) fn equal_pipelines_of(
        &self,
        device: &wgpu::Device,
        arrays: bool,
        scene: SceneMode,
        format: wgpu::TextureFormat,
        fade: u8,
        keys: &[PipelineKey],
    ) -> HashMap<PipelineKey, wgpu::RenderPipeline> {
        use rayon::prelude::*;
        let at = std::panic::Location::caller();
        let mode =
            self.mode_modern(device, arrays, false, scene.lamps, scene.modern, scene.bias_q != 0);
        let id = |k: PipelineKey| (arrays, scene, format, fade, k);
        let missing: Vec<PipelineKey> = {
            let built = self.built_equal.lock().unwrap();
            keys.iter().copied().filter(|&k| !built.contains_key(&id(k))).collect()
        };
        let made: Vec<(PipelineKey, wgpu::RenderPipeline)> = missing
            .into_par_iter()
            .map(|k| {
                (k, {
                    note_pipeline_at(at, self.tag);
                    make_pipeline_with(
                        device,
                        &mode.layout,
                        &mode.shader,
                        k,
                        format,
                        wgpu::ColorWrites::ALL,
                        None,
                        scene,
                        self.cache.as_ref(),
                        fade,
                        true,
                        false,
                    )
                })
            })
            .collect();
        let mut built = self.built_equal.lock().unwrap();
        for (k, p) in made {
            built.entry(id(k)).or_insert(p);
        }
        keys.iter().map(|&k| (k, built[&id(k)].clone())).collect()
    }

    /// [`WorldShared::equal_pipelines`]' worldreflect keys ([`mrt_keys`])
    /// with the SSR mask's two targets after the scene's
    /// ([`make_pipeline_with`]'s `mrt`): the main view's opaque road with
    /// the mask from the main view (docs/wet-roads-ssr.md).
    pub(crate) fn mrt_pipelines(
        &self,
        device: &wgpu::Device,
        arrays: bool,
        scene: SceneMode,
        format: wgpu::TextureFormat,
        fade: u8,
    ) -> HashMap<PipelineKey, wgpu::RenderPipeline> {
        let at = std::panic::Location::caller();
        let mode =
            self.mode_modern(device, arrays, false, scene.lamps, scene.modern, scene.bias_q != 0);
        let keys = mrt_keys();
        let id = |k: PipelineKey| (arrays, scene, format, fade, k);
        let missing: Vec<PipelineKey> = {
            let built = self.built_mrt.lock().unwrap();
            keys.iter().copied().filter(|&k| !built.contains_key(&id(k))).collect()
        };
        let made: Vec<(PipelineKey, wgpu::RenderPipeline)> = missing
            .into_iter()
            .map(|k| {
                note_pipeline_at(at, self.tag);
                let p = make_pipeline_with(
                    device,
                    &mode.layout,
                    &mode.shader,
                    k,
                    format,
                    wgpu::ColorWrites::ALL,
                    None,
                    scene,
                    self.cache.as_ref(),
                    fade,
                    true,
                    true,
                );
                (k, p)
            })
            .collect();
        let mut built = self.built_mrt.lock().unwrap();
        for (k, p) in made {
            built.entry(id(k)).or_insert(p);
        }
        keys.iter().map(|&k| (k, built[&id(k)].clone())).collect()
    }
}

/// The keys the main view's opaque road takes ([`WorldShared::mrt_pipelines`]):
/// worldreflect, unblended, with and without the alpha test.
pub(crate) fn mrt_keys() -> Vec<PipelineKey> {
    [false, true].map(|alpha_test| PipelineKey { effect: Effect::Reflect, blend: Blend::None, alpha_test }).to_vec()
}

/// A depth-only pipeline of the main view's pre-pass: the colour
/// pipeline's vertex entry point, depth state and sample count
/// ([`make_pipeline`]), its alpha test and LOD Fade mask, no colour.
fn make_depth_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    key: DepthKey,
    mode: SceneMode,
    cache: Option<&wgpu::PipelineCache>,
) -> wgpu::RenderPipeline {
    let vertex_attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Unorm8x4, 2 => Float32x2];
    let instance_attributes = wgpu::vertex_attr_array![3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4];
    let fade_attributes = wgpu::vertex_attr_array![7 => Float32];
    let buffers = [
        Some(wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &vertex_attributes,
        }),
        Some(wgpu::VertexBufferLayout {
            array_stride: 64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &instance_attributes,
        }),
        Some(wgpu::VertexBufferLayout {
            array_stride: 4,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &fade_attributes,
        }),
    ];
    let vertex_entry = match (key.reflect, key.fade > 0) {
        (false, false) => "vs_main",
        (true, false) => "vs_reflect",
        (false, true) => "vs_main_fade",
        (true, true) => "vs_reflect_fade",
    };
    let fragment_entry = match (key.alpha_test, key.fade) {
        (false, 2) => Some("fs_depth_fade"),
        (true, 2) => Some("fs_depth_alpha_test_fade"),
        (true, _) => Some("fs_depth_alpha_test"),
        (false, _) => None,
    };
    let mut constants = mode.fragment_constants();
    constants.extend(mode.lamp_constants());
    crate::render::core::device_cache::timed_render_pipeline(device, &wgpu::RenderPipelineDescriptor {
        label: Some("depth pre-pass"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some(vertex_entry),
            compilation_options: Default::default(),
            buffers: &buffers[..if key.fade > 0 { 3 } else { 2 }],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(mode.compare()),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: wgpu::MultisampleState { count: mode.samples, ..Default::default() },
        fragment: fragment_entry.map(|e| wgpu::FragmentState {
            module: shader,
            entry_point: Some(e),
            compilation_options: wgpu::PipelineCompilationOptions { constants: &constants, ..Default::default() },
            targets: &[],
        }),
        multiview_mask: None,
        cache,
    })
}

pub(crate) fn make_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    key: PipelineKey,
    format: wgpu::TextureFormat,
    write_mask: wgpu::ColorWrites,
    multiview_mask: Option<std::num::NonZeroU32>,
    mode: SceneMode,
    cache: Option<&wgpu::PipelineCache>,
    // LOD Fade (clone deviation): 0 none (native), 1 the instance's fade
    // read and a 0 dropped (the native fragment shader: early depth kept),
    // 2 also the screen-door dither (a sample mask).
    fade: u8,
) -> wgpu::RenderPipeline {
    make_pipeline_with(device, layout, shader, key, format, write_mask, multiview_mask, mode, cache, fade, false, false)
}

/// SRCBLEND / DESTBLEND for a texture's blend byte (+0x56), as 0x5BCC40
/// sets them (**verified**: switch table 0x5BCF90, 0x5BCCD2…0x5BCD24, then
/// 0x5BCBD0 = ALPHABLENDENABLE / SRCBLEND / DESTBLEND): 0 → blending off;
/// 1 → SRCALPHA / INVSRCALPHA; 2 → SRCALPHA / ONE; 5 → DESTALPHA /
/// INVDESTALPHA; 6 → DESTALPHA / ONE (3, 4 and > 6 → SRCALPHA /
/// INVSRCALPHA, which the format reads as `Alpha`). None for no blending.
/// The same factors blend alpha (no SEPARATEALPHABLENDENABLE).
pub(crate) fn native_blend_factors(blend: Blend) -> Option<(wgpu::BlendFactor, wgpu::BlendFactor)> {
    use wgpu::BlendFactor as F;
    match blend {
        Blend::None => None,
        Blend::Alpha => Some((F::SrcAlpha, F::OneMinusSrcAlpha)),
        Blend::Additive => Some((F::SrcAlpha, F::One)),
        Blend::DestAlpha => Some((F::DstAlpha, F::OneMinusDstAlpha)),
        Blend::DestAlphaAdditive => Some((F::DstAlpha, F::One)),
    }
}

/// [`make_pipeline`]; with `equal` (after the depth pre-pass) the
/// depth test is EQUAL and depth is not written; with `mrt` (worldreflect
/// keys only) the `_mrt` entry, which also writes the SSR mask's term and
/// surface (two more targets, [`WET_MASK_FORMAT`]: docs/wet-roads-ssr.md
/// "The mask from the main view").
#[allow(clippy::too_many_arguments)]
fn make_pipeline_with(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    key: PipelineKey,
    format: wgpu::TextureFormat,
    write_mask: wgpu::ColorWrites,
    multiview_mask: Option<std::num::NonZeroU32>,
    mode: SceneMode,
    cache: Option<&wgpu::PipelineCache>,
    fade: u8,
    equal: bool,
    mrt: bool,
) -> wgpu::RenderPipeline {
    let format = mode.target(format);
    // D3D blend factors as set by 0x5BCC40 ([`native_blend_factors`]). The back buffer is A8R8G8B8
    // (BackBufferFormat 0x7FEE68 = 21, 0x5B9AF1), and nothing enables
    // SEPARATEALPHABLENDENABLE, so D3D blends alpha with the colour factors;
    // that alpha is the bleach bypass mask (screenfilter.efx p0).
    let factors = |(src, dst): (wgpu::BlendFactor, wgpu::BlendFactor)| {
        let blend = wgpu::BlendComponent { src_factor: src, dst_factor: dst, operation: wgpu::BlendOperation::Add };
        Some(wgpu::BlendState { color: blend, alpha: blend })
    };
    let (color_blend, blended) = (native_blend_factors(key.blend).and_then(factors), key.blend != Blend::None);
    let entry_point = match (key.effect, key.alpha_test) {
        (Effect::SkyMorph, _) => "fs_skymorph",
        (Effect::World, false) => "fs_world",
        (Effect::World, true) => "fs_world_alpha_test",
        (Effect::NoFog, false) => "fs_nofog",
        (Effect::NoFog, true) => "fs_nofog_alpha_test",
        (Effect::PlanarNoFog, false) => "fs_planar_nofog",
        (Effect::PlanarNoFog, true) => "fs_planar_nofog_alpha_test",
        (Effect::Reflect, false) => "fs_reflect",
        (Effect::Reflect, true) => "fs_reflect_alpha_test",
        (Effect::CarShadow, false) => "fs_car_shadow",
        (Effect::CarShadow, true) => "fs_car_shadow_alpha_test",
    };
    // Alpha-to-coverage (the Graphics page's MSAA): the alpha test's edge
    // as a sample mask.
    let entry_point = match (mode.coverage, entry_point) {
        (true, "fs_world_alpha_test") => "fs_world_coverage",
        (true, "fs_nofog_alpha_test") => "fs_nofog_coverage",
        (true, "fs_reflect_alpha_test") => "fs_reflect_coverage",
        (_, e) => e,
    };
    let mut constants = mode.fragment_constants();
    constants.extend(mode.lamp_constants());
    let vertex_attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Unorm8x4, 2 => Float32x2];
    let instance_attributes = wgpu::vertex_attr_array![3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4];
    // LOD Fade's pipelines (clone deviation): the same with each instance's
    // fade (location 7) and the `_fade` entry points (with MSAA's
    // alpha-to-coverage, `fs_*_coverage_fade`: both sample masks ANDed).
    let fade_attributes = wgpu::vertex_attr_array![7 => Float32];
    let vertex_entry = if key.effect == Effect::Reflect { "vs_reflect" } else { "vs_main" };
    let vertex_entry = if fade > 0 { format!("{vertex_entry}_fade") } else { vertex_entry.to_string() };
    let entry_point = if fade > 1 { format!("{entry_point}_fade") } else { entry_point.to_string() };
    let entry_point = if mrt { format!("{entry_point}_mrt") } else { entry_point };
    let mask_target = Some(wgpu::ColorTargetState { format: WET_MASK_FORMAT, blend: None, write_mask: wgpu::ColorWrites::ALL });
    let main_target = Some(wgpu::ColorTargetState { format, blend: color_blend, write_mask });
    let targets = if mrt { vec![main_target, mask_target.clone(), mask_target] } else { vec![main_target] };
    let buffers = [
        Some(wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &vertex_attributes,
        }),
        Some(wgpu::VertexBufferLayout {
            array_stride: 64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &instance_attributes,
        }),
        Some(wgpu::VertexBufferLayout {
            array_stride: 4,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &fade_attributes,
        }),
    ];
    crate::render::core::device_cache::timed_render_pipeline(device, &wgpu::RenderPipelineDescriptor {
        label: Some(&entry_point),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some(&vertex_entry),
            compilation_options: Default::default(),
            buffers: &buffers[..if fade > 0 { 3 } else { 2 }],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            // world.efx pass p0: CULLMODE = NONE.
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(!blended && !equal),
            // world.efx: ZFUNC = LESSEQUAL (GREATEREQUAL reversed).
            depth_compare: Some(if equal { wgpu::CompareFunction::Equal } else { mode.compare() }),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: wgpu::MultisampleState { count: mode.samples, ..Default::default() },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(&entry_point),
            compilation_options: wgpu::PipelineCompilationOptions { constants: &constants, ..Default::default() },
            targets: &targets,
        }),
        multiview_mask,
        cache,
    })
}

/// The draws (ascending, deduplicated) of `placements`' instance groups:
/// what [`WorldCore::overlay_draws`] holds for the main-view-only overlays.
pub(crate) fn overlay_draws(
    placements: impl IntoIterator<Item = usize>,
    placement_slots: &[Vec<u32>],
    slot_group: &[u32],
    group_draws: &[Vec<u32>],
) -> Vec<u32> {
    let mut draws: Vec<u32> = placements
        .into_iter()
        .filter_map(|p| placement_slots.get(p))
        .flatten()
        .filter_map(|&slot| slot_group.get(slot as usize))
        .filter_map(|&g| group_draws.get(g as usize))
        .flatten()
        .copied()
        .collect();
    draws.sort_unstable();
    draws.dedup();
    draws
}

/// Screen-Space Reflections' mask pipeline ([`WetMask`]): worldreflect's
/// vertex shader (the main view's, so the same pixels) into the mask's
/// two targets, depth-tested against the mask's own depth.
/// Wet Roads' `Frame.wet` from (rain amount, the road's wetness from the
/// rain, world clock): zero (the native path) until the rain has wetted
/// the road, so a dry scene draws as the original (docs/wet-roads-ssr.md).
pub(crate) fn wet_uniform((rain, road_wet, clock): (f32, f32, f32)) -> [f32; 4] {
    if road_wet <= WET_MIN { [0.0; 4] } else { [1.0, rain, road_wet, clock] }
}

/// The road wetness under which Wet Roads draws nothing (the native path).
pub(crate) const WET_MIN: f32 = 1.0e-3;

pub(crate) fn make_wet_mask_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    entry: &str,
    reversed: bool,
    cache: Option<&wgpu::PipelineCache>,
) -> wgpu::RenderPipeline {
    let vertex_attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Unorm8x4, 2 => Float32x2];
    let instance_attributes = wgpu::vertex_attr_array![3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4];
    // (`fs_wet_mask_kind*`: the tint alone, the term and the surface the
    // main view's, docs/wet-roads-ssr.md "The mask from the main view".)
    let writes = if entry.starts_with("fs_wet_mask_kind") { wgpu::ColorWrites::empty() } else { wgpu::ColorWrites::ALL };
    let target = Some(wgpu::ColorTargetState { format: WET_MASK_FORMAT, blend: None, write_mask: writes });
    let tint =
        Some(wgpu::ColorTargetState { format: WET_MASK_TINT_FORMAT, blend: None, write_mask: wgpu::ColorWrites::ALL });
    crate::render::core::device_cache::timed_render_pipeline(device, &wgpu::RenderPipelineDescriptor {
        label: Some(entry),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_reflect"),
            compilation_options: Default::default(),
            buffers: &[
                Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &vertex_attributes,
                }),
                Some(wgpu::VertexBufferLayout {
                    array_stride: 64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &instance_attributes,
                }),
            ],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(if reversed {
                wgpu::CompareFunction::GreaterEqual
            } else {
                wgpu::CompareFunction::LessEqual
            }),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(entry),
            compilation_options: Default::default(),
            targets: &[target.clone(), target, tint],
        }),
        multiview_mask: None,
        cache,
    })
}

/// The SSR mask's hidden road (world_masks.wgsl `fs_wet_mask_hidden*`):
/// tested against the main view's depth, passing only what lies behind
/// it (GREATER, LESS reversed), not writing depth; the term and the
/// surface written, the tint kept (the kind pass's).
pub(crate) fn make_wet_mask_hidden_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    entry: &str,
    reversed: bool,
    cache: Option<&wgpu::PipelineCache>,
) -> wgpu::RenderPipeline {
    let vertex_attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Unorm8x4, 2 => Float32x2];
    let instance_attributes = wgpu::vertex_attr_array![3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4];
    let target =
        Some(wgpu::ColorTargetState { format: WET_MASK_FORMAT, blend: None, write_mask: wgpu::ColorWrites::ALL });
    let tint = Some(wgpu::ColorTargetState {
        format: WET_MASK_TINT_FORMAT,
        blend: None,
        write_mask: wgpu::ColorWrites::empty(),
    });
    crate::render::core::device_cache::timed_render_pipeline(device, &wgpu::RenderPipelineDescriptor {
        label: Some(entry),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_reflect"),
            compilation_options: Default::default(),
            buffers: &[
                Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &vertex_attributes,
                }),
                Some(wgpu::VertexBufferLayout {
                    array_stride: 64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &instance_attributes,
                }),
            ],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(false),
            depth_compare: Some(if reversed { wgpu::CompareFunction::Less } else { wgpu::CompareFunction::Greater }),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(entry),
            compilation_options: Default::default(),
            targets: &[target.clone(), target, tint],
        }),
        multiview_mask: None,
        cache,
    })
}

/// An animation mask pipeline ([`AnimMask`]): the main view's
/// vertex entry and the mask's fragment entry (a coverage, scaled by the
/// blend constant: the draw's values), depth-tested against (and written
/// to) the mask's own depth.
#[allow(clippy::too_many_arguments)]
pub(crate) fn make_anim_mask_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    vertex: &str,
    fragment: &str,
    reversed: bool,
    cache: Option<&wgpu::PipelineCache>,
) -> wgpu::RenderPipeline {
    let vertex_attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Unorm8x4, 2 => Float32x2];
    let instance_attributes = wgpu::vertex_attr_array![3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4];
    let scale = wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::Constant,
        dst_factor: wgpu::BlendFactor::Zero,
        operation: wgpu::BlendOperation::Add,
    };
    crate::render::core::device_cache::timed_render_pipeline(device, &wgpu::RenderPipelineDescriptor {
        label: Some(fragment),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some(vertex),
            compilation_options: Default::default(),
            buffers: &[
                Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &vertex_attributes,
                }),
                Some(wgpu::VertexBufferLayout {
                    array_stride: 64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &instance_attributes,
                }),
            ],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(if reversed {
                wgpu::CompareFunction::GreaterEqual
            } else {
                wgpu::CompareFunction::LessEqual
            }),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(fragment),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: ANIM_MASK_FORMAT,
                blend: Some(wgpu::BlendState { color: scale, alpha: scale }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache,
    })
}

#[cfg(test)]
mod lamp_source_tests {
    /// Without the lamps the world module is the native shader: no lamp
    /// code, no lamp bindings used (docs/lamps-shadows.md).
    #[test]
    fn world_source_strips_the_lamps() {
        let native = crate::render::world_source(false);
        let lamps = crate::render::world_source(true);
        // The SSR mask's Surface Detail (the mapped normal) is the lamps' module's only.
        for s in ["lamp_light", "contact_visibility", "shadow_maps", "fs_shadow_alpha_test", "fs_car_shadow", "BEGIN LAMPS", "frame.lamps", "SURFACE_SSR_DRY"] {
            assert!(!native.contains(s), "{s} in the native module");
            assert!(lamps.contains(s), "{s} missing from the lamps' module");
        }
        // The composer's LAMPS sections (core::shader) hold all of it, each
        // whole: none in the native module, every one closed in the lamps'.
        assert!(!native.contains("LAMPS") && !native.lines().chain(lamps.lines()).any(|l| l.trim_start().starts_with('#')));
        assert_eq!(lamps.matches("// BEGIN LAMPS").count(), lamps.matches("// END LAMPS").count());
    }

    /// The shadow texture's contact variant is never asked of the
    /// native module (not among the scene's keys), and only for
    /// alpha-blended shadow textures.
    #[test]
    fn car_shadow_effect_only_in_the_lamps_module() {
        assert!(crate::render::PipelineKey::all().iter().all(|k| k.effect != crate::render::Effect::CarShadow));
        assert!(crate::render::car_shadow_keys().iter().all(|k| k.effect == crate::render::Effect::CarShadow && k.blend == crate::render::Blend::Alpha));
    }
}

#[cfg(test)]
mod depth_prepass_tests {
    use crate::render::{Blend, Effect, PipelineKey, SceneMode};
    use wgpu::naga;

    /// The depth pre-pass and the specialisation constants are the lamps' module's only: the native module has no
    /// pre-pass entry points, no invariant position and no specialisation constants.
    #[test]
    fn native_module_untouched() {
        let native = crate::render::world_source_modern(false, 0);
        for s in ["fs_depth_", "@invariant", "D9_"] {
            assert!(!native.contains(s), "{s} in the native module");
        }
        for modern in 0..3 {
            let lamps = crate::render::world_source_modern(true, modern);
            for s in ["fn fs_depth_alpha_test(", "fn fs_depth_fade(", "fn fs_depth_alpha_test_fade(", "@builtin(position) @invariant clip"] {
                assert!(lamps.contains(s), "{s} missing (modern {modern})");
            }
            // Both vertex outputs invariant (the colour and fade pipelines').
            assert_eq!(lamps.matches("@invariant").count(), 2);
        }
    }

    /// The pre-pass takes the opaque draws only (not the sky, the blends,
    /// skymorph, the car shadow decals), and the alpha-tested ones only
    /// without alpha-to-coverage.
    #[test]
    fn prepass_draws() {
        for k in PipelineKey::all() {
            let opaque = k.blend == Blend::None && k.effect != Effect::SkyMorph;
            assert_eq!(crate::render::in_depth_prepass(k, false, false), opaque, "{k:?}");
            assert_eq!(crate::render::in_depth_prepass(k, false, true), opaque && !k.alpha_test, "{k:?}");
            assert!(!crate::render::in_depth_prepass(k, true, false));
        }
        assert!(crate::render::depth_prepass_keys().iter().all(|k| k.blend == Blend::None));
        assert_eq!(crate::render::depth_prepass_keys().len(), 6);
        assert!(!crate::render::depth_prepass(SceneMode::NATIVE));
    }

    /// Every constant a lamps' pipeline gets is an override of its
    /// module (wgpu rejects unknown ones), none without the lamps, and the
    /// module validates with them for every feature set.
    #[test]
    fn lamp_constants_fit_the_module() {
        assert!(SceneMode::NATIVE.lamp_constants().is_empty());
        for (modern, subgroups) in [(0u8, false), (1, false), (0, true), (1, true)] {
            let source = crate::render::world_module_source_for(true, modern, true, false, false, false, subgroups);
            let module = naga::front::wgsl::parse_str(&source).expect("parse");
            let info = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all()).validate(&module).expect("valid");
            for features in [0u16, 0x1F | (2 << crate::render::LF_SHADOW_Q_SHIFT), crate::render::LF_LAMPS | crate::render::LF_SHADOWS | (1 << crate::render::LF_SHADOW_Q_SHIFT) | crate::render::LF_ML_LOW] {
                let mode = SceneMode { lamps: true, modern, lamp_features: features, ..SceneMode::NATIVE };
                let constants = mode.lamp_constants();
                assert_eq!(constants.iter().any(|(k, _)| k.starts_with("D9_ML_")), modern > 0);
                let mut map: naga::back::PipelineConstants = Default::default();
                for (k, v) in &constants {
                    assert!(module.overrides.iter().any(|(_, o)| o.name.as_deref() == Some(*k)), "{k} not an override");
                    map.insert(k.to_string(), *v);
                }
                for entry in ["fs_world", "fs_reflect_alpha_test", "fs_world_alpha_test_fade", "fs_depth_alpha_test"] {
                    naga::back::pipeline_constants::process_overrides(&module, &info, Some((naga::ShaderStage::Fragment, entry)), &map).expect(entry);
                }
            }
        }
    }
}

#[cfg(test)]
mod wet_roads_tests {
    use crate::render::*;

    /// Wet Roads draws the native path (wet.x = 0) until the rain has
    /// wetted the road; then the rain, the road's wetness and the world
    /// clock (the ripples' time, so shots show them at the scene's clock).
    #[test]
    fn wet_uniform_is_native_on_a_dry_road() {
        assert_eq!(wet_uniform((0.0, 0.0, 170.0)), [0.0; 4]);
        assert_eq!(wet_uniform((0.8, WET_MIN, 170.0)), [0.0; 4]);
        assert_eq!(wet_uniform((0.8, 0.5, 170.0)), [1.0, 0.8, 0.5, 170.0]);
        // A road drying after the rain stopped keeps its wet look.
        assert_eq!(wet_uniform((0.0, 0.3, 12.0)), [1.0, 0.0, 0.3, 12.0]);
    }
}

#[cfg(test)]
mod scene_mode_tests {
    use crate::render::*;

    #[test]
    fn native_mode_keeps_the_format_and_hdr_takes_half_float() {
        let f = wgpu::TextureFormat::Bgra8Unorm;
        assert_eq!(SceneMode::NATIVE.target(f), f);
        assert_eq!(SceneMode { samples: 4, reversed: true, coverage: true, hdr: false, lamps: false, modern: 0, bias_q: 0, lamp_features: 0 }.target(f), f);
        assert_eq!(SceneMode { hdr: true, ..SceneMode::NATIVE }.target(f), HDR_FORMAT);
        assert_ne!(SceneMode { hdr: true, ..SceneMode::NATIVE }, SceneMode::NATIVE);
    }
}

#[cfg(test)]
mod blend_factor_tests {
    use super::native_blend_factors;
    use crate::formats::texture::Blend;
    use wgpu::BlendFactor as F;

    /// 0x5BCC40's factors per blend byte 0–6 (jump table 0x5BCF90; bytes
    /// 5 and 6 are the destination-alpha pair).
    #[test]
    fn blend_bytes_give_the_originals_factors() {
        let want = [
            None,
            Some((F::SrcAlpha, F::OneMinusSrcAlpha)),
            Some((F::SrcAlpha, F::One)),
            Some((F::SrcAlpha, F::OneMinusSrcAlpha)),
            Some((F::SrcAlpha, F::OneMinusSrcAlpha)),
            Some((F::DstAlpha, F::OneMinusDstAlpha)),
            Some((F::DstAlpha, F::One)),
        ];
        for (byte, want) in want.into_iter().enumerate() {
            assert_eq!(native_blend_factors(Blend::from_entry_byte(byte as u8)), want, "blend byte {byte}");
        }
    }
}
