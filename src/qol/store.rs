use super::*;

impl Enhancements {
    /// Put these values in force.
    pub fn apply(&self) {
        self.apply_now();
    }

    /// [`Enhancements::apply`] at once: the values in force replaced in the
    /// settings store in one change (a reader sees the old values or the
    /// new ones, never a mix), then the table's rows in their `apply` order
    /// (`options!`: the rows that call a setter elsewhere).
    pub fn apply_now(&self) {
        // docs/settings-model.md: the values in force are the
        // stored ones with the settings model's engine rules applied (a
        // redundant or conflicting row resolved); options.txt keeps the
        // stored values, so turning the overriding row off restores them.
        let stored = self;
        let e = &stored.effective();
        change(|s| s.values = *e);
        for r in apply_order() {
            (r.apply)(e);
        }
        if stored.crash_reports {
            record("crash::keep_log", &());
            crash::keep_log();
        }
    }
}

/// The settings store: the Enhancements values in force and the facts of
/// the machine and the session they are read with. One per process; every
/// getter of this module reads through it ([`read`]) and every setter
/// changes it one change at a time ([`change`]).
#[derive(Clone, Copy, Debug)]
pub(super) struct InForce {
    /// The values in force: the stored ones with the engine rules applied
    /// ([`Enhancements::apply_now`]); native until the first apply.
    pub(super) values: Enhancements,
    /// The MSAA sample counts the device offers (bit `log2 n`; 1x always).
    /// Before the GPU is made every choice is visible; `Gpu::new` installs
    /// the actual support.
    pub(super) msaa_mask: u8,
    /// The adapter's largest 2D texture edge and whether it is a small GPU
    /// (integrated or software) or a software one ([`set_gpu_caps`]).
    pub(super) max_dim: u32,
    pub(super) small_gpu: bool,
    pub(super) software_gpu: bool,
    /// The actual window/offscreen output (width << 32 | height; 0
    /// unknown), distinct from the requested fullscreen resolution
    /// (borderless fallback and resizes can change it).
    pub(super) output_size: u64,
    /// docs/settings-page.md: the settings page's Compare — while set,
    /// `Settings::apply_enhancements` puts the Original preset's graphics
    /// values in force (QoL kept) instead of the stored ones; nothing is
    /// changed or saved. Session state, cleared when the page is left.
    pub(super) compare: bool,
    /// 0 Integrated, 1 Mid, 2 High (the default until a GPU is made: tools
    /// and tests see the presets as before).
    pub(super) gpu_class: u8,
    /// The frame being drawn runs the temporal pass ([`set_temporal_frame`]).
    pub(super) temporal_frame: bool,
    /// The device renders, blends, filters and resolves the half-float scene
    /// at every sample count it offers ([`set_hdr_support`]).
    pub(super) hdr_supported: bool,
    /// The signal the window's surface offers (0 none, 1 PQ, 2 scRGB).
    pub(super) hdr_signal: u8,
    /// What the display reports (nits, 0 unknown): peak, SDR white.
    pub(super) hdr_display_peak: u16,
    pub(super) hdr_display_paper: u16,
    /// The calibration pattern's rectangle in the output (fractions of its
    /// width and height: left, top, right, bottom) while the HDR Calibration
    /// page is open; None: no pattern.
    pub(super) hdr_pattern: Option<[f32; 4]>,
    /// The device was made with ray queries (`render::rt::wanted`).
    pub(super) rt_supported: bool,
    /// The adapter can trace (`render::rt::adapter_can`): the tier is
    /// offered (in force after a restart when the device was made without).
    pub(super) rt_offered: bool,
}

impl InForce {
    /// The store before anything is applied or set: native values, no
    /// device facts yet.
    fn initial() -> InForce {
        InForce {
            values: Enhancements::default(),
            msaa_mask: 31,
            max_dim: 8192,
            small_gpu: false,
            software_gpu: false,
            output_size: 0,
            compare: false,
            gpu_class: 2,
            temporal_frame: false,
            hdr_supported: true,
            hdr_signal: 0,
            hdr_display_peak: 0,
            hdr_display_paper: 0,
            hdr_pattern: None,
            rt_supported: false,
            rt_offered: false,
        }
    }
}

static STORE: std::sync::LazyLock<std::sync::RwLock<InForce>> =
    std::sync::LazyLock::new(|| std::sync::RwLock::new(InForce::initial()));

/// Reads the store. `f` must not call a getter of this module (they read
/// the store themselves): take what it needs from `s` alone.
#[inline]
pub(super) fn read<R>(f: impl FnOnce(&InForce) -> R) -> R {
    f(&STORE.read().unwrap_or_else(std::sync::PoisonError::into_inner))
}

/// Changes the store: one change at a time. `f` must not call a getter or
/// setter of this module.
pub(super) fn change(f: impl FnOnce(&mut InForce)) {
    f(&mut STORE.write().unwrap_or_else(std::sync::PoisonError::into_inner))
}

/// A row's value in force as `apply_now`'s log notes it
/// (`apply [N store NAME]`): the integer FOV offset as a 32-bit value,
/// the rest as they are.
pub(super) trait Logged: Copy {
    type As: std::fmt::Debug;
    fn logged(self) -> Self::As;
}

macro_rules! logged_same {
    ($($t:ty),*) => {$(
        impl Logged for $t {
            type As = $t;
            fn logged(self) -> $t {
                self
            }
        }
    )*};
}
logged_same!(bool, u8, u16);

impl Logged for i8 {
    type As = i32;
    fn logged(self) -> i32 {
        i32::from(self)
    }
}

/// Notes a row's value in force under its name (the test hook
/// [`APPLY_LOG`]; nothing outside tests).
#[inline(always)]
pub(super) fn record_value<T: Logged>(name: &'static str, v: T) {
    record(name, &v.logged());
}

#[cfg(test)]
thread_local! {
    /// Test hook: while a test has set it, `apply_now` notes each
    /// value it puts in force (under its name) and each setter it calls, in order.
    pub(super) static APPLY_LOG: std::cell::RefCell<Option<Vec<(&'static str, String)>>> =
        const { std::cell::RefCell::new(None) };
}

#[inline(always)]
pub(super) fn record(name: &'static str, v: &dyn std::fmt::Debug) {
    #[cfg(test)]
    APPLY_LOG.with(|l| {
        if let Some(l) = l.borrow_mut().as_mut() {
            l.push((name, format!("{v:?}")));
        }
    });
    #[cfg(not(test))]
    let _ = (name, v);
}

/// Modern Lighting's strength by default (%).
pub const MODERN_STRENGTH_DEFAULT: u8 = 100;
/// The strength row's steps (%).
pub const MODERN_STRENGTH_STEPS: [u8; 5] = [0, 25, 50, 75, 100];

/// Modern Lighting as chosen (0 Original, 1 Low, 2 Modern, 3 Modern +
/// RT, whether or not the device can trace).
pub fn modern_lighting_requested() -> u8 {
    read(|s| s.values.modern_lighting)
}

/// Modern Lighting in force: 3 only while the device traces (else 2).
pub fn modern_lighting() -> u8 {
    let v = read(|s| s.values.modern_lighting);
    if v == 3 && !rt_supported() { 2 } else { v }
}

/// Modern Lighting's strength, 0..=100 (%).
pub fn modern_strength() -> u8 {
    read(|s| s.values.modern_strength)
}
pub fn set_msaa_support(mask: u8) {
    change(|s| s.msaa_mask = mask | 1);
}
pub fn supported_msaa(wanted: u32) -> u32 {
    let mask = read(|s| s.msaa_mask);
    [16, 8, 4, 2, 1].into_iter().find(|&n| n <= wanted && mask & (1 << n.ilog2()) != 0).unwrap_or(1)
}
pub fn msaa_offered(n: u8) -> bool {
    n <= 1 || supported_msaa(n as u32) == n as u32
}

/// Smooth Car Shapes in force (read when a car is loaded).
pub fn smooth_cars() -> bool {
    read(|s| s.values.smooth_cars)
}

/// World Detail Extreme in force (see [`Enhancements::far_draw`]).
pub fn far_draw() -> bool {
    read(|s| s.values.far_draw)
}

/// World Detail Extreme's ring (metres): the drivable outlines within it
/// lend their native visible lists (Ultra's is `world::LOOKAHEAD_RING`).
pub const FAR_RING: f32 = 400.0;

/// Traffic Density in force: 0 Native, 1 More, 2 Dense (read when free
/// roam's traffic is loaded).
pub fn traffic_density() -> u8 {
    read(|s| s.values.traffic_density)
}

pub const TRAFFIC_DENSITY_STEPS: [u8; 3] = [0, 1, 2];


/// Photo Mode in Pause Menu in force (see [`Enhancements::photo_mode`]).
pub fn photo_mode() -> bool {
    read(|s| s.values.photo_mode)
}

/// Fix Original Model Errors in force (see [`Enhancements::fix_models`];
/// read when a car is loaded).
pub fn fix_models() -> bool {
    read(|s| s.values.fix_models)
}

pub fn set_output_size(size: (u32, u32)) {
    let packed = if size.0 == 0 || size.1 == 0 { 0 } else { (u64::from(size.0) << 32) | u64::from(size.1) };
    change(|s| s.output_size = packed);
}

pub fn output_size_or(requested: (u32, u32)) -> (u32, u32) {
    let size = read(|s| s.output_size);
    if size == 0 { requested } else { ((size >> 32) as u32, size as u32) }
}

/// The adapter's limits the Graphics page's caps follow.
pub fn set_gpu_caps(max_texture_dimension: u32, device_type: wgpu::DeviceType) {
    change(|s| {
        s.max_dim = max_texture_dimension.max(1);
        s.small_gpu = matches!(device_type, wgpu::DeviceType::IntegratedGpu | wgpu::DeviceType::Cpu);
        s.software_gpu = device_type == wgpu::DeviceType::Cpu;
    });
}

/// The settings page's Compare is on ([`InForce::compare`]).
pub fn comparing() -> bool {
    read(|s| s.compare)
}

pub fn set_compare(on: bool) {
    change(|s| s.compare = on);
}

/// Set by `render::Gpu::new`; `NFSU2_GPU_CLASS=integrated|mid|high`
/// overrides it (shots, benches).
pub fn set_gpu_class(class: GpuClass) {
    let class = match std::env::var("NFSU2_GPU_CLASS").as_deref() {
        Ok("integrated" | "igpu") => GpuClass::Integrated,
        Ok("mid") => GpuClass::Mid,
        Ok("high") => GpuClass::High,
        _ => class,
    };
    change(|s| s.gpu_class = class as u8);
}

pub fn gpu_class() -> GpuClass {
    match read(|s| s.gpu_class) {
        0 => GpuClass::Integrated,
        1 => GpuClass::Mid,
        _ => GpuClass::High,
    }
}

/// A software adapter (llvmpipe, WARP): the page's costly rows are greyed.
pub fn software_gpu() -> bool {
    read(|s| s.software_gpu)
}

/// The frame being drawn runs the temporal pass (set by the draw): the
/// game's world with TAA / Upscaling. Front-end and garage scenes with TAA
/// chosen are drawn without it, at the native MSAA.
pub fn set_temporal_frame(on: bool) {
    change(|s| s.temporal_frame = on);
}

pub fn temporal_frame() -> bool {
    read(|s| s.temporal_frame)
}

/// Volumetric Fog in force: 0 off, 1 Low, 2 High, 3 Next-Gen.
pub fn volumetric_fog() -> u8 {
    read(|s| s.values.volumetric_fog)
}

/// Sky in force: 0 Original, 1 Next-Gen.
pub fn sky() -> u8 {
    read(|s| s.values.sky)
}

/// Particles in force: 0 Original, 1 High, 2 Ultra.
pub fn particles() -> u8 {
    read(|s| s.values.particles)
}

/// Sparks & Fire in force: 0 Original, 1 Next-Gen.
pub fn sparks_fire() -> u8 {
    read(|s| s.values.sparks_fire)
}

/// Weather (docs/weather.md): Wet Car in force.
pub fn wet_car() -> bool {
    read(|s| s.values.wet_car)
}

/// Rain Detail in force: 0 off, 1 On, 2 High.
pub fn rain_detail() -> u8 {
    read(|s| s.values.rain_detail)
}

/// Rain Detail's Next-Gen tier (docs/rain-nextgen.md): the
/// simulated rain volume, world-anchored splashes on every up-facing
/// surface, rain haze and impacts on the cars.
pub const RAIN_NEXT_GEN: u8 = 3;

/// Rain Detail at Next-Gen.
pub fn rain_next_gen() -> bool {
    rain_detail() >= RAIN_NEXT_GEN
}

/// Spray & Splashes in force.
pub fn spray() -> bool {
    read(|s| s.values.spray)
}

/// Lens Drops in force.
pub fn lens_drops() -> bool {
    read(|s| s.values.lens_drops)
}

/// Wet Road Interaction: 0 off, 1 On, 2 High.
pub fn wet_interaction() -> u8 {
    read(|s| s.values.wet_interaction)
}

/// Any weather row on (the in-game frame then publishes the weather).
pub fn weather_fx() -> bool {
    wet_car() || rain_detail() != 0 || spray() || lens_drops() || wet_interaction() != 0
}

/// Motion Blur's clone tier in force: 0 native, 1 Camera, 2
/// Per-Object.
pub fn motion_blur() -> u8 {
    read(|s| s.values.motion_blur)
}

/// Depth Of Field's clone tier in force: 0 native, 1 Cinematic.
pub fn depth_of_field() -> u8 {
    read(|s| s.values.depth_of_field)
}

/// Animated Water in force (docs/water.md).
pub fn animated_water() -> bool {
    read(|s| s.values.water) != 0
}

/// Ambient Detail's tier (docs/ambient-detail.md): 0 Off, 1 On (the
/// street props), 2 Next-Gen (also blown debris, tyre dust, steam, moths).
pub fn ambient_detail() -> u8 {
    read(|s| s.values.ambient_detail)
}

/// Animated Water's tier (docs/water.md): 0 Off, 1 On,
/// 2 Realistic (`render::water_hq`), 3 Next-Gen (`render::water_ng`).
pub fn water_tier() -> u8 {
    read(|s| s.values.water)
}

/// Headlight Beams in force (docs/lighting2.md).
pub fn headlight_beams() -> bool {
    read(|s| s.values.headlight_beams) != 0
}

/// docs/car-lights.md: the Headlight Beams row at Next-Gen.
pub fn car_lights_next_gen() -> bool {
    read(|s| s.values.headlight_beams) >= 2
}

/// Light Probes in force (docs/lighting2.md).
pub fn light_probes() -> bool {
    read(|s| s.values.light_probes)
}

/// Enhanced Glass in force (docs/lighting2.md).
pub fn enhanced_glass() -> bool {
    read(|s| s.values.enhanced_glass)
}

/// Surface Detail in force (docs/surface-maps.md): 0 off, 1
/// Materials, 2 Materials + Normals.
pub fn surface_detail() -> u8 {
    read(|s| s.values.surface_detail)
}

/// SMAA 1x in force.
pub fn smaa() -> bool {
    !temporal() && filters(post_aa(), read(|s| s.values.aa_filter)).1
}

/// Texture Filtering's anisotropy in force (0 = native).
pub fn anisotropy() -> u16 {
    u16::from(read(|s| s.values.anisotropy))
}

/// High Precision Post asked for (0 off, 1 half-size glow, 2 full-size);
/// HDR Output in force needs its float chain (at least 1).
pub fn hdr_post() -> u8 {
    let v = read(|s| s.values.hdr_post);
    if hdr_output_active() { v.max(1) } else { v }
}

/// Reversed-Z depth asked for.
pub fn reversed_z() -> bool {
    read(|s| s.values.reversed_z)
}

/// Shadow Quality in force (0 native, 1 high, 2 ultra).
pub fn shadow_quality() -> u8 {
    read(|s| s.values.shadows)
}

/// Optional enhanced car shading and filtered, higher-detail road capture.
pub fn enhanced_reflections() -> bool {
    read(|s| s.values.enhanced_reflections)
}

/// Enhanced Car Paint in force (clearcoat, flake, pearl shift; off = native).
/// Lists Open on Installed in force (see [`Enhancements::lists_open_installed`]).
pub fn lists_open_installed() -> bool {
    read(|s| s.values.lists_open_installed)
}

/// Outrun Variety in force (see [`Enhancements::outrun_variety`]).
pub fn outrun_variety() -> bool {
    read(|s| s.values.outrun_variety)
}

/// Outrun Variety on or off alone (tests; `Enhancements::apply` sets it
/// with the rest).
pub fn set_outrun_variety(on: bool) {
    change(|s| s.values.outrun_variety = on);
}

pub fn enhanced_paint() -> bool {
    read(|s| s.values.enhanced_paint)
}

/// docs/car-scene-specular.md: the cars' harmonic highlight comes
/// from the scene's lights only, not from the in-game rig's camera-fixed
/// lights (0x7F3F00), whose highlight sits on whatever panel faces the
/// camera (the trunk from the chase camera). The rule
/// ([`scene_lights_car`], the values in force): on in the higher Enhanced
/// settings and with ray tracing; off (the native highlight, untouched)
/// in the Original and the lower Enhanced settings.
pub fn scene_car_specular() -> bool {
    scene_lights_car(CarShadingRows {
        modern_lighting: modern_lighting(),
        ssr: ssr(),
        enhanced_paint: enhanced_paint(),
        enhanced_reflections: enhanced_reflections(),
        light_probes: light_probes(),
    })
}

/// The rows [`scene_lights_car`] reads (their values in force: a
/// ray-traced tier counts only while the device traces).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CarShadingRows {
    /// 0 Original, 1 Low, 2 Modern, 3 Modern + ray-traced shadows.
    pub modern_lighting: u8,
    /// 0 off, 1 half, 2 full, 3 ray-traced reflections.
    pub ssr: u8,
    pub enhanced_paint: bool,
    pub enhanced_reflections: bool,
    pub light_probes: bool,
}

/// The rule (docs/car-scene-specular.md "Which
/// settings"): whether the scene lights the car, so the native rig's
/// camera-fixed highlight goes (true), or the native highlight stays
/// exactly as in SPEED2.EXE (false). Higher (true): Modern Lighting at
/// any tier; any ray-traced tier (Modern Lighting's or the reflections');
/// Enhanced Car Paint or Enhanced Reflections together with the scene's
/// light on the car — Light Probes, or Screen-Space Reflections (the
/// scene reflected in the paint; the High / Ultra / Extreme
/// presets and a reported settings combination). Lower (false): the Original and
/// the cheap rows alone (Balanced: Paint; Paint / Reflections without
/// probes or SSR; every other row).
pub fn scene_lights_car(r: CarShadingRows) -> bool {
    let ray_traced = r.modern_lighting == 3 || r.ssr == 3;
    let car_shading = r.enhanced_paint || r.enhanced_reflections;
    r.modern_lighting != 0 || ray_traced || (car_shading && (r.light_probes || r.ssr != 0))
}

/// Reflection Update Rate "Every Frame" in force.
pub fn reflection_every_frame() -> bool {
    read(|s| s.values.reflection_rate) != 0
}

/// Wet Roads in force (docs/wet-roads-ssr.md).
pub fn wet_roads() -> bool {
    read(|s| s.values.wet_roads)
}

/// Screen-Space Reflections in force: 0 off, 1 half size, 2 full size, 3
/// full size + ray-traced (only while the device can trace, else 2).
pub fn ssr() -> u8 {
    let v = read(|s| s.values.ssr);
    if v == 3 && !rt_supported() { 2 } else { v }
}

pub fn set_rt_offered(ok: bool) {
    change(|s| s.rt_offered = ok);
}

/// The Ray Traced tier can be chosen on this machine.
pub fn rt_offered() -> bool {
    read(|s| s.rt_offered) || rt_supported()
}

/// The Screen-Space Reflections value as chosen (3 whether or not the
/// device can trace).
pub fn ssr_requested() -> u8 {
    read(|s| s.values.ssr)
}

pub fn set_rt_support(ok: bool) {
    change(|s| s.rt_supported = ok);
}

/// The device can trace rays: the Ray Traced tier is offered.
pub fn rt_supported() -> bool {
    read(|s| s.rt_supported)
}

/// Ray-traced reflections in force (docs/rt-reflections.md).
pub fn rt_reflections() -> bool {
    ssr() == 3
}

/// Dynamic Lamps in force (docs/lamps-shadows.md).
pub fn dynamic_lamps() -> bool {
    read(|s| s.values.dynamic_lamps)
}

/// Lamp Shadows in force: 0 off, 1 Low, 2 High (docs/lamps-shadows.md).
pub fn lamp_shadows() -> u8 {
    read(|s| s.values.lamp_shadows)
}

/// Contact Shadows in force (docs/lamps-shadows.md).
pub fn contact_shadows() -> bool {
    read(|s| s.values.contact_shadows)
}

/// The ambient occlusion's strength in force, 0 (off) to 1.
pub fn ssao() -> f32 {
    f32::from(read(|s| s.values.ssao)) / 100.0
}

/// The CAS sharpness in force, 0 (off) to 1.
pub fn sharpen() -> f32 {
    f32::from(read(|s| s.values.sharpen)) / 100.0
}

/// The road reflection image's size for a scene `scene` big: native 320
/// × 240 (`native`), half the scene's or the scene's own.
pub fn road_reflection_size(native: (u32, u32), scene: (u32, u32)) -> (u32, u32) {
    road_reflection_size_for(read(|s| s.values.road_reflection), native, scene)
}

/// [`road_reflection_size`] for Road Reflection Res `v`.
pub fn road_reflection_size_for(v: u8, native: (u32, u32), scene: (u32, u32)) -> (u32, u32) {
    match v {
        0 => native,
        _ if scene.0 == 0 || scene.1 == 0 => native,
        1 => ((scene.0 / 2).max(native.0), (scene.1 / 2).max(native.1)),
        _ => (scene.0.max(native.0), scene.1.max(native.1)),
    }
}

/// The cars' environment cube's edge in force (128 native).
pub fn car_reflection_size() -> u32 {
    u32::from(read(|s| s.values.car_reflection)).clamp(128, 1024)
}

/// Detail Distance in force, as a factor (1 native).
pub fn detail_distance() -> f32 {
    f32::from(read(|s| s.values.detail_distance)) / 100.0
}

/// Car Detail Distance in force, as a factor (1 native).
pub fn car_detail() -> f32 {
    f32::from(read(|s| s.values.car_detail)) / 100.0
}

/// LOD Fade in force (false native).
pub fn lod_fade() -> bool {
    read(|s| s.values.lod_fade)
}

/// Keep All Loaded in force (false native).
pub fn keep_loaded() -> bool {
    read(|s| s.values.keep_loaded)
}

/// docs/pop-in.md: World Detail Ultra / Extreme's pop-in fixes are
/// in force (Keep All Loaded on; `NFSU2_G14_OFF`, a measuring aid, keeps
/// the tiers as they were before the pop-in work for A/B runs on one binary).
pub fn pop_free() -> bool {
    static OFF: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    keep_loaded() && !*OFF.get_or_init(|| std::env::var_os("NFSU2_G14_OFF").is_some())
}

/// LOD Fade's length, seconds: an instance crossing the size cull or a
/// car changing LOD goes from not drawn to drawn (or back) over this.
pub const LOD_FADE_SECONDS: f32 = 0.25;
/// LOD Fade's hysteresis: a drawn scenery instance stays drawn down to
/// this fraction of the size it needs to appear (17 px × the Detail
/// Distance's factor); a car keeps its LOD while its size is within this
/// factor of the threshold either way.
pub const LOD_FADE_KEEP: f32 = 0.85;
/// docs/pop-in.md: at World Detail Ultra / Extreme with LOD Fade, a
/// whole section joining or leaving the drawn set (a visible-list change,
/// the look-ahead ring, the far set) cross-fades over this long instead of
/// [`LOD_FADE_SECONDS`]: the two copies of a place share the pixels by the
/// screen-door pattern, so the swap reads as a slow dissolve.
pub const SECTION_FADE_SECONDS: f32 = 1.0;
pub const CAR_LOD_HYSTERESIS: f32 = 1.1;

/// The player camera's field-of-view offset in force, radians (0 native).
/// HD UI Art in force ([`Enhancements::hd_ui`]).
pub fn hd_ui() -> bool {
    read(|s| s.values.hd_ui)
}

/// Reload Packs When Changed in force ([`Enhancements::reload_packs`]).
pub fn reload_packs() -> bool {
    read(|s| s.values.reload_packs)
}

pub fn fov_offset() -> f32 {
    (read(|s| s.values.fov_offset) as f32).to_radians()
}

/// The HUD scale in force (1 native).
pub fn hud_scale() -> f32 {
    f32::from(read(|s| s.values.hud_scale)) / 100.0
}

/// The frame cap in force, if any.
pub fn frame_cap() -> Option<std::time::Duration> {
    match read(|s| s.values.frame_cap) {
        0 => None,
        fps => Some(std::time::Duration::from_secs_f64(1.0 / f64::from(fps))),
    }
}

pub fn durable_saves() -> bool {
    read(|s| s.values.durable_saves)
}

pub fn save_backups() -> usize {
    read(|s| s.values.save_backups) as usize
}

pub fn crash_reports() -> bool {
    read(|s| s.values.crash_reports)
}

#[cfg(test)]
mod car_highlight_rule_tests {
    use super::*;

    fn rows(e: &Enhancements) -> CarShadingRows {
        CarShadingRows {
            modern_lighting: e.modern_lighting,
            ssr: e.ssr,
            enhanced_paint: e.enhanced_paint,
            enhanced_reflections: e.enhanced_reflections,
            light_probes: e.light_probes,
        }
    }

    /// The table (docs/car-scene-specular.md "Which settings") over
    /// the presets at every GPU class and the car-shading rows alone and
    /// in pairs: Original and the lower settings keep the native rig
    /// highlight, the higher ones and every ray-traced tier remove it.
    #[test]
    fn the_rig_highlight_follows_the_table() {
        for class in [GpuClass::High, GpuClass::Mid, GpuClass::Integrated] {
            for p in PRESETS {
                let mut e = Enhancements::default();
                p.apply_for(class, &mut e);
                let higher = !matches!(p, Preset::Original | Preset::Clean);
                assert_eq!(scene_lights_car(rows(&e)), higher, "{} ({class:?})", p.name());
            }
        }
        let r = CarShadingRows::default;
        let lower = [
            r(),
            CarShadingRows { enhanced_paint: true, ..r() },
            CarShadingRows { enhanced_reflections: true, ..r() },
            CarShadingRows { enhanced_paint: true, enhanced_reflections: true, ..r() },
            CarShadingRows { light_probes: true, ..r() },
            CarShadingRows { ssr: 2, ..r() },
        ];
        for x in lower {
            assert!(!scene_lights_car(x), "lower: {x:?}");
        }
        let higher = [
            CarShadingRows { modern_lighting: 1, ..r() },
            CarShadingRows { modern_lighting: 2, ..r() },
            CarShadingRows { modern_lighting: 3, ..r() },
            CarShadingRows { ssr: 3, ..r() },
            CarShadingRows { enhanced_paint: true, light_probes: true, ..r() },
            CarShadingRows { enhanced_reflections: true, light_probes: true, ..r() },
            CarShadingRows { enhanced_paint: true, ssr: 1, ..r() },
            CarShadingRows { enhanced_reflections: true, ssr: 2, ..r() },
        ];
        for x in higher {
            assert!(scene_lights_car(x), "higher: {x:?}");
        }
    }

    /// A reported settings combination (Enhanced Paint + Enhanced Reflections + SSR Ultra, no probes, no
    /// Modern Lighting) counts as higher.
    #[test]
    fn reported_settings_are_higher() {
        let reported = CarShadingRows { ssr: 2, enhanced_paint: true, enhanced_reflections: true, ..Default::default() };
        assert!(scene_lights_car(reported));
    }
}
