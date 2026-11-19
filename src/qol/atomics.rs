use super::*;

impl Enhancements {
    /// Put these values in force.
    pub fn apply(&self) {
        self.apply_now();
    }

    /// [`Enhancements::apply`] at once: each row's value stored in its
    /// global, in the table's `apply` order (`options!`).
    pub fn apply_now(&self) {
        // docs/settings-model.md: the values in force are the
        // stored ones with the settings model's engine rules applied (a
        // redundant or conflicting row resolved); options.txt keeps the
        // stored values, so turning the overriding row off restores them.
        let stored = self;
        let e = &stored.effective();
        for r in apply_order() {
            (r.apply)(e);
        }
        if stored.crash_reports {
            record("crash::keep_log", &());
            crash::keep_log();
        }
    }
}

/// A global that holds one option's value in force (`apply [N atom …]`).
pub(super) trait Put<T> {
    fn put(&self, name: &'static str, v: T);
}

macro_rules! put_same {
    ($($atomic:ty => $t:ty),*) => {$(
        impl Put<$t> for $atomic {
            fn put(&self, name: &'static str, v: $t) {
                record(name, &v);
                self.store(v, Ordering::Relaxed);
            }
        }
    )*};
}
put_same!(AtomicBool => bool, AtomicU8 => u8, AtomicU16 => u16);

impl Put<i8> for AtomicI32 {
    fn put(&self, name: &'static str, v: i8) {
        let v = v as i32;
        record(name, &v);
        self.store(v, Ordering::Relaxed);
    }
}

#[cfg(test)]
thread_local! {
    /// Test hook: while a test has set it, `apply_now` notes each
    /// global it stores and the value, in order.
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

pub(super) static FOV_OFFSET: AtomicI32 = AtomicI32::new(0);
pub(super) static HUD_SCALE: AtomicU8 = AtomicU8::new(100);
pub(super) static FRAME_CAP: AtomicU16 = AtomicU16::new(0);
pub(super) static DURABLE: AtomicBool = AtomicBool::new(false);
pub(super) static BACKUPS: AtomicU8 = AtomicU8::new(0);
pub(super) static CRASH: AtomicBool = AtomicBool::new(false);
pub(super) static RENDER_SCALE: AtomicU8 = AtomicU8::new(100);
pub(super) static POST_AA: AtomicU8 = AtomicU8::new(0);
pub(super) static UPSCALE: AtomicU8 = AtomicU8::new(0);
pub(super) static VOLUMETRIC_FOG: AtomicU8 = AtomicU8::new(0);
pub(super) static SKY: AtomicU8 = AtomicU8::new(0);
pub(super) static PARTICLES: AtomicU8 = AtomicU8::new(0);
pub(super) static SPARKS_FIRE: AtomicU8 = AtomicU8::new(0);
pub(super) static WET_CAR: AtomicBool = AtomicBool::new(false);
pub(super) static RAIN_DETAIL: AtomicU8 = AtomicU8::new(0);
pub(super) static SPRAY: AtomicBool = AtomicBool::new(false);
pub(super) static LENS_DROPS: AtomicBool = AtomicBool::new(false);
pub(super) static WET_INTERACTION: AtomicU8 = AtomicU8::new(0);
pub(super) static MOTION_BLUR: AtomicU8 = AtomicU8::new(0);
pub(super) static DEPTH_OF_FIELD: AtomicU8 = AtomicU8::new(0);
pub(super) static WATER: AtomicU8 = AtomicU8::new(0);
pub(super) static AMBIENT_DETAIL: AtomicU8 = AtomicU8::new(0);
pub(super) static HEADLIGHT_BEAMS: AtomicU8 = AtomicU8::new(0);
pub(super) static LIGHT_PROBES: AtomicBool = AtomicBool::new(false);
pub(super) static ENHANCED_GLASS: AtomicBool = AtomicBool::new(false);
pub(super) static SURFACE_DETAIL: AtomicU8 = AtomicU8::new(0);
pub(super) static MODERN_LIGHTING: AtomicU8 = AtomicU8::new(0);
pub(super) static MODERN_STRENGTH: AtomicU8 = AtomicU8::new(MODERN_STRENGTH_DEFAULT);
/// Modern Lighting's strength by default (%).
pub const MODERN_STRENGTH_DEFAULT: u8 = 100;
/// The strength row's steps (%).
pub const MODERN_STRENGTH_STEPS: [u8; 5] = [0, 25, 50, 75, 100];

/// Modern Lighting as chosen (0 Original, 1 Low, 2 Modern, 3 Modern +
/// RT, whether or not the device can trace).
pub fn modern_lighting_requested() -> u8 {
    MODERN_LIGHTING.load(Ordering::Relaxed)
}

/// Modern Lighting in force: 3 only while the device traces (else 2).
pub fn modern_lighting() -> u8 {
    let v = MODERN_LIGHTING.load(Ordering::Relaxed);
    if v == 3 && !rt_supported() { 2 } else { v }
}

/// Modern Lighting's strength, 0..=100 (%).
pub fn modern_strength() -> u8 {
    MODERN_STRENGTH.load(Ordering::Relaxed)
}
pub(super) static AA_MSAA: AtomicU8 = AtomicU8::new(0);
pub(super) static AA_FILTER: AtomicU8 = AtomicU8::new(0);
// Prior to GPU initialization, all choices are visible. Gpu::new installs actual support.
pub(super) static MSAA_MASK: AtomicU8 = AtomicU8::new(31);
pub fn set_msaa_support(mask: u8) {
    MSAA_MASK.store(mask | 1, Ordering::Relaxed);
}
pub fn supported_msaa(wanted: u32) -> u32 {
    [16, 8, 4, 2, 1]
        .into_iter()
        .find(|&n| n <= wanted && MSAA_MASK.load(Ordering::Relaxed) & (1 << n.ilog2()) != 0)
        .unwrap_or(1)
}
pub fn msaa_offered(n: u8) -> bool {
    n <= 1 || supported_msaa(n as u32) == n as u32
}

pub(super) static SHARPEN: AtomicU8 = AtomicU8::new(0);
pub(super) static ROAD_REFLECTION: AtomicU8 = AtomicU8::new(0);
pub(super) static CAR_REFLECTION: AtomicU16 = AtomicU16::new(128);
pub(super) static DETAIL_DISTANCE: AtomicU16 = AtomicU16::new(100);
pub(super) static CAR_DETAIL: AtomicU16 = AtomicU16::new(100);
pub(super) static HD_UI: AtomicBool = AtomicBool::new(false);
pub(super) static RELOAD_PACKS: AtomicBool = AtomicBool::new(false);
pub(super) static LOD_FADE: AtomicBool = AtomicBool::new(false);
pub(super) static KEEP_LOADED: AtomicBool = AtomicBool::new(false);
pub(super) static ANISOTROPY: AtomicU8 = AtomicU8::new(0);
pub(super) static REVERSED_Z: AtomicBool = AtomicBool::new(false);
pub(super) static SHADOWS: AtomicU8 = AtomicU8::new(0);
pub(super) static ENHANCED_REFLECTIONS: AtomicBool = AtomicBool::new(false);
pub(super) static REFLECTION_RATE: AtomicU8 = AtomicU8::new(0);
pub(super) static SSAO: AtomicU8 = AtomicU8::new(0);
pub(super) static HDR_POST: AtomicU8 = AtomicU8::new(0);
pub(super) static WET_ROADS: AtomicBool = AtomicBool::new(false);
pub(super) static SSR: AtomicU8 = AtomicU8::new(0);
pub(super) static DYNAMIC_LAMPS: AtomicBool = AtomicBool::new(false);
pub(super) static LAMP_SHADOWS: AtomicU8 = AtomicU8::new(0);
pub(super) static CONTACT_SHADOWS: AtomicBool = AtomicBool::new(false);
/// The device renders, blends, filters and resolves the half-float scene
/// at every sample count it offers ([`set_hdr_support`]).
pub(super) static HDR_SUPPORTED: AtomicBool = AtomicBool::new(true);
pub(super) static ENHANCED_PAINT: AtomicBool = AtomicBool::new(false);
pub(super) static LISTS_OPEN_INSTALLED: AtomicBool = AtomicBool::new(false);
pub(super) static OUTRUN_VARIETY: AtomicBool = AtomicBool::new(false);
pub(super) static FIX_MODELS: AtomicBool = AtomicBool::new(false);
pub(super) static PHOTO_MODE: AtomicBool = AtomicBool::new(false);
pub(super) static SMOOTH_CARS: AtomicBool = AtomicBool::new(false);
pub(super) static FAR_DRAW: AtomicBool = AtomicBool::new(false);
pub(super) static TRAFFIC_DENSITY: AtomicU8 = AtomicU8::new(0);

/// Smooth Car Shapes in force (read when a car is loaded).
pub fn smooth_cars() -> bool {
    SMOOTH_CARS.load(Ordering::Relaxed)
}

/// World Detail Extreme in force (see [`Enhancements::far_draw`]).
pub fn far_draw() -> bool {
    FAR_DRAW.load(Ordering::Relaxed)
}

/// World Detail Extreme's ring (metres): the drivable outlines within it
/// lend their native visible lists (Ultra's is `world::LOOKAHEAD_RING`).
pub const FAR_RING: f32 = 400.0;

/// Traffic Density in force: 0 Native, 1 More, 2 Dense (read when free
/// roam's traffic is loaded).
pub fn traffic_density() -> u8 {
    TRAFFIC_DENSITY.load(Ordering::Relaxed)
}

pub const TRAFFIC_DENSITY_STEPS: [u8; 3] = [0, 1, 2];


/// Photo Mode in Pause Menu in force (see [`Enhancements::photo_mode`]).
pub fn photo_mode() -> bool {
    PHOTO_MODE.load(Ordering::Relaxed)
}

/// Fix Original Model Errors in force (see [`Enhancements::fix_models`];
/// read when a car is loaded).
pub fn fix_models() -> bool {
    FIX_MODELS.load(Ordering::Relaxed)
}
/// The adapter's largest 2D texture edge and whether it is a small GPU
/// (integrated or software), set by `render::Gpu::new` ([`set_gpu_caps`]).
pub(super) static MAX_DIM: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(8192);
pub(super) static SMALL_GPU: AtomicBool = AtomicBool::new(false);
pub(super) static SOFTWARE_GPU: AtomicBool = AtomicBool::new(false);
/// Actual window/offscreen output, distinct from the requested fullscreen
/// resolution (borderless fallback and resizes can change it).
pub(super) static OUTPUT_SIZE: AtomicU64 = AtomicU64::new(0);

pub fn set_output_size(size: (u32, u32)) {
    let packed = if size.0 == 0 || size.1 == 0 { 0 } else { (u64::from(size.0) << 32) | u64::from(size.1) };
    OUTPUT_SIZE.store(packed, Ordering::Relaxed);
}

pub fn output_size_or(requested: (u32, u32)) -> (u32, u32) {
    let size = OUTPUT_SIZE.load(Ordering::Relaxed);
    if size == 0 { requested } else { ((size >> 32) as u32, size as u32) }
}

/// The adapter's limits the Graphics page's caps follow.
pub fn set_gpu_caps(max_texture_dimension: u32, device_type: wgpu::DeviceType) {
    MAX_DIM.store(max_texture_dimension.max(1), Ordering::Relaxed);
    SMALL_GPU.store(matches!(device_type, wgpu::DeviceType::IntegratedGpu | wgpu::DeviceType::Cpu), Ordering::Relaxed);
    SOFTWARE_GPU.store(device_type == wgpu::DeviceType::Cpu, Ordering::Relaxed);
}

/// docs/settings-page.md: the settings page's Compare — while set,
/// `Settings::apply_enhancements` puts the Original preset's graphics
/// values in force (QoL kept) instead of the stored ones; nothing is
/// changed or saved. Session state, cleared when the page is left.
pub(super) static COMPARE: AtomicBool = AtomicBool::new(false);

pub fn comparing() -> bool {
    COMPARE.load(Ordering::Relaxed)
}

pub fn set_compare(on: bool) {
    COMPARE.store(on, Ordering::Relaxed);
}

/// 0 Integrated, 1 Mid, 2 High (the default until a GPU is made: tools and
/// tests see the presets as before).
pub(super) static GPU_CLASS: AtomicU8 = AtomicU8::new(2);

/// Set by `render::Gpu::new`; `NFSU2_GPU_CLASS=integrated|mid|high`
/// overrides it (shots, benches).
pub fn set_gpu_class(class: GpuClass) {
    let class = match std::env::var("NFSU2_GPU_CLASS").as_deref() {
        Ok("integrated" | "igpu") => GpuClass::Integrated,
        Ok("mid") => GpuClass::Mid,
        Ok("high") => GpuClass::High,
        _ => class,
    };
    GPU_CLASS.store(class as u8, Ordering::Relaxed);
}

pub fn gpu_class() -> GpuClass {
    match GPU_CLASS.load(Ordering::Relaxed) {
        0 => GpuClass::Integrated,
        1 => GpuClass::Mid,
        _ => GpuClass::High,
    }
}

/// A software adapter (llvmpipe, WARP): the page's costly rows are greyed.
pub fn software_gpu() -> bool {
    SOFTWARE_GPU.load(Ordering::Relaxed)
}

pub(super) static TEMPORAL_FRAME: AtomicBool = AtomicBool::new(false);

/// The frame being drawn runs the temporal pass (set by the draw): the
/// game's world with TAA / Upscaling. Front-end and garage scenes with TAA
/// chosen are drawn without it, at the native MSAA.
pub fn set_temporal_frame(on: bool) {
    TEMPORAL_FRAME.store(on, Ordering::Relaxed);
}

pub fn temporal_frame() -> bool {
    TEMPORAL_FRAME.load(Ordering::Relaxed)
}

/// Volumetric Fog in force: 0 off, 1 Low, 2 High, 3 Next-Gen.
pub fn volumetric_fog() -> u8 {
    VOLUMETRIC_FOG.load(Ordering::Relaxed)
}

/// Sky in force: 0 Original, 1 Next-Gen.
pub fn sky() -> u8 {
    SKY.load(Ordering::Relaxed)
}

/// Particles in force: 0 Original, 1 High, 2 Ultra.
pub fn particles() -> u8 {
    PARTICLES.load(Ordering::Relaxed)
}

/// Sparks & Fire in force: 0 Original, 1 Next-Gen.
pub fn sparks_fire() -> u8 {
    SPARKS_FIRE.load(Ordering::Relaxed)
}

/// Weather (docs/weather.md): Wet Car in force.
pub fn wet_car() -> bool {
    WET_CAR.load(Ordering::Relaxed)
}

/// Rain Detail in force: 0 off, 1 On, 2 High.
pub fn rain_detail() -> u8 {
    RAIN_DETAIL.load(Ordering::Relaxed)
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
    SPRAY.load(Ordering::Relaxed)
}

/// Lens Drops in force.
pub fn lens_drops() -> bool {
    LENS_DROPS.load(Ordering::Relaxed)
}

/// Wet Road Interaction: 0 off, 1 On, 2 High.
pub fn wet_interaction() -> u8 {
    WET_INTERACTION.load(Ordering::Relaxed)
}

/// Any weather row on (the in-game frame then publishes the weather).
pub fn weather_fx() -> bool {
    wet_car() || rain_detail() != 0 || spray() || lens_drops() || wet_interaction() != 0
}

/// Motion Blur's clone tier in force: 0 native, 1 Camera, 2
/// Per-Object.
pub fn motion_blur() -> u8 {
    MOTION_BLUR.load(Ordering::Relaxed)
}

/// Depth Of Field's clone tier in force: 0 native, 1 Cinematic.
pub fn depth_of_field() -> u8 {
    DEPTH_OF_FIELD.load(Ordering::Relaxed)
}

/// Animated Water in force (docs/water.md).
pub fn animated_water() -> bool {
    WATER.load(Ordering::Relaxed) != 0
}

/// Ambient Detail's tier (docs/ambient-detail.md): 0 Off, 1 On (the
/// street props), 2 Next-Gen (also blown debris, tyre dust, steam, moths).
pub fn ambient_detail() -> u8 {
    AMBIENT_DETAIL.load(Ordering::Relaxed)
}

/// Animated Water's tier (docs/water.md): 0 Off, 1 On,
/// 2 Realistic (`render::water_hq`), 3 Next-Gen (`render::water_ng`).
pub fn water_tier() -> u8 {
    WATER.load(Ordering::Relaxed)
}

/// Headlight Beams in force (docs/lighting2.md).
pub fn headlight_beams() -> bool {
    HEADLIGHT_BEAMS.load(Ordering::Relaxed) != 0
}

/// docs/car-lights.md: the Headlight Beams row at Next-Gen.
pub fn car_lights_next_gen() -> bool {
    HEADLIGHT_BEAMS.load(Ordering::Relaxed) >= 2
}

/// Light Probes in force (docs/lighting2.md).
pub fn light_probes() -> bool {
    LIGHT_PROBES.load(Ordering::Relaxed)
}

/// Enhanced Glass in force (docs/lighting2.md).
pub fn enhanced_glass() -> bool {
    ENHANCED_GLASS.load(Ordering::Relaxed)
}

/// Surface Detail in force (docs/surface-maps.md): 0 off, 1
/// Materials, 2 Materials + Normals.
pub fn surface_detail() -> u8 {
    SURFACE_DETAIL.load(Ordering::Relaxed)
}

/// SMAA 1x in force.
pub fn smaa() -> bool {
    !temporal() && filters(post_aa(), AA_FILTER.load(Ordering::Relaxed)).1
}

/// Texture Filtering's anisotropy in force (0 = native).
pub fn anisotropy() -> u16 {
    u16::from(ANISOTROPY.load(Ordering::Relaxed))
}

/// High Precision Post asked for (0 off, 1 half-size glow, 2 full-size);
/// HDR Output in force needs its float chain (at least 1).
pub fn hdr_post() -> u8 {
    let v = HDR_POST.load(Ordering::Relaxed);
    if hdr_output_active() { v.max(1) } else { v }
}

pub(super) static HDR_OUTPUT: AtomicBool = AtomicBool::new(false);
pub(super) static HDR_PEAK: AtomicU16 = AtomicU16::new(0);
pub(super) static HDR_PAPER: AtomicU16 = AtomicU16::new(0);
pub(super) static HDR_GAMMA: AtomicU8 = AtomicU8::new(0);
/// The signal the window's surface offers (0 none, 1 PQ, 2 scRGB).
pub(super) static HDR_SIGNAL: AtomicU8 = AtomicU8::new(0);
/// What the display reports (nits, 0 unknown): peak, SDR white.
pub(super) static HDR_DISPLAY_PEAK: AtomicU16 = AtomicU16::new(0);
pub(super) static HDR_DISPLAY_PAPER: AtomicU16 = AtomicU16::new(0);
/// The calibration pattern's rectangle in the output (fractions of its
/// width and height: left, top, right, bottom) while the HDR Calibration
/// page is open; None: no pattern.
pub(super) static HDR_PATTERN: std::sync::Mutex<Option<[f32; 4]>> = std::sync::Mutex::new(None);
/// Reversed-Z depth asked for.
pub fn reversed_z() -> bool {
    REVERSED_Z.load(Ordering::Relaxed)
}

/// Shadow Quality in force (0 native, 1 high, 2 ultra).
pub fn shadow_quality() -> u8 {
    SHADOWS.load(Ordering::Relaxed)
}

/// Optional enhanced car shading and filtered, higher-detail road capture.
pub fn enhanced_reflections() -> bool {
    ENHANCED_REFLECTIONS.load(Ordering::Relaxed)
}

/// Enhanced Car Paint in force (clearcoat, flake, pearl shift; off = native).
/// Lists Open on Installed in force (see [`Enhancements::lists_open_installed`]).
pub fn lists_open_installed() -> bool {
    LISTS_OPEN_INSTALLED.load(Ordering::Relaxed)
}

/// Outrun Variety in force (see [`Enhancements::outrun_variety`]).
pub fn outrun_variety() -> bool {
    OUTRUN_VARIETY.load(Ordering::Relaxed)
}

/// Outrun Variety on or off alone (tests; `Enhancements::apply` sets it
/// with the rest).
pub fn set_outrun_variety(on: bool) {
    OUTRUN_VARIETY.store(on, Ordering::Relaxed);
}

pub fn enhanced_paint() -> bool {
    ENHANCED_PAINT.load(Ordering::Relaxed)
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
    REFLECTION_RATE.load(Ordering::Relaxed) != 0
}

/// Wet Roads in force (docs/wet-roads-ssr.md).
pub fn wet_roads() -> bool {
    WET_ROADS.load(Ordering::Relaxed)
}

/// Screen-Space Reflections in force: 0 off, 1 half size, 2 full size, 3
/// full size + ray-traced (only while the device can trace, else 2).
pub fn ssr() -> u8 {
    let v = SSR.load(Ordering::Relaxed);
    if v == 3 && !rt_supported() { 2 } else { v }
}

/// The device was made with ray queries (`render::rt::wanted`).
pub(super) static RT_SUPPORTED: AtomicBool = AtomicBool::new(false);
/// The adapter can trace (`render::rt::adapter_can`): the tier is
/// offered (in force after a restart when the device was made without).
pub(super) static RT_OFFERED: AtomicBool = AtomicBool::new(false);

pub fn set_rt_offered(ok: bool) {
    RT_OFFERED.store(ok, Ordering::Relaxed);
}

/// The Ray Traced tier can be chosen on this machine.
pub fn rt_offered() -> bool {
    RT_OFFERED.load(Ordering::Relaxed) || rt_supported()
}

/// The Screen-Space Reflections value as chosen (3 whether or not the
/// device can trace).
pub fn ssr_requested() -> u8 {
    SSR.load(Ordering::Relaxed)
}

pub fn set_rt_support(ok: bool) {
    RT_SUPPORTED.store(ok, Ordering::Relaxed);
}

/// The device can trace rays: the Ray Traced tier is offered.
pub fn rt_supported() -> bool {
    RT_SUPPORTED.load(Ordering::Relaxed)
}

/// Ray-traced reflections in force (docs/rt-reflections.md).
pub fn rt_reflections() -> bool {
    ssr() == 3
}

/// Dynamic Lamps in force (docs/lamps-shadows.md).
pub fn dynamic_lamps() -> bool {
    DYNAMIC_LAMPS.load(Ordering::Relaxed)
}

/// Lamp Shadows in force: 0 off, 1 Low, 2 High (docs/lamps-shadows.md).
pub fn lamp_shadows() -> u8 {
    LAMP_SHADOWS.load(Ordering::Relaxed)
}

/// Contact Shadows in force (docs/lamps-shadows.md).
pub fn contact_shadows() -> bool {
    CONTACT_SHADOWS.load(Ordering::Relaxed)
}

/// The ambient occlusion's strength in force, 0 (off) to 1.
pub fn ssao() -> f32 {
    f32::from(SSAO.load(Ordering::Relaxed)) / 100.0
}

/// The CAS sharpness in force, 0 (off) to 1.
pub fn sharpen() -> f32 {
    f32::from(SHARPEN.load(Ordering::Relaxed)) / 100.0
}

/// The road reflection image's size for a scene `scene` big: native 320
/// × 240 (`native`), half the scene's or the scene's own.
pub fn road_reflection_size(native: (u32, u32), scene: (u32, u32)) -> (u32, u32) {
    road_reflection_size_for(ROAD_REFLECTION.load(Ordering::Relaxed), native, scene)
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
    u32::from(CAR_REFLECTION.load(Ordering::Relaxed)).clamp(128, 1024)
}

/// Detail Distance in force, as a factor (1 native).
pub fn detail_distance() -> f32 {
    f32::from(DETAIL_DISTANCE.load(Ordering::Relaxed)) / 100.0
}

/// Car Detail Distance in force, as a factor (1 native).
pub fn car_detail() -> f32 {
    f32::from(CAR_DETAIL.load(Ordering::Relaxed)) / 100.0
}

/// LOD Fade in force (false native).
pub fn lod_fade() -> bool {
    LOD_FADE.load(Ordering::Relaxed)
}

/// Keep All Loaded in force (false native).
pub fn keep_loaded() -> bool {
    KEEP_LOADED.load(Ordering::Relaxed)
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
    HD_UI.load(Ordering::Relaxed)
}

/// Reload Packs When Changed in force ([`Enhancements::reload_packs`]).
pub fn reload_packs() -> bool {
    RELOAD_PACKS.load(Ordering::Relaxed)
}

pub fn fov_offset() -> f32 {
    (FOV_OFFSET.load(Ordering::Relaxed) as f32).to_radians()
}

/// The HUD scale in force (1 native).
pub fn hud_scale() -> f32 {
    f32::from(HUD_SCALE.load(Ordering::Relaxed)) / 100.0
}

/// The frame cap in force, if any.
pub fn frame_cap() -> Option<std::time::Duration> {
    match FRAME_CAP.load(Ordering::Relaxed) {
        0 => None,
        fps => Some(std::time::Duration::from_secs_f64(1.0 / f64::from(fps))),
    }
}

pub fn durable_saves() -> bool {
    DURABLE.load(Ordering::Relaxed)
}

pub fn save_backups() -> usize {
    BACKUPS.load(Ordering::Relaxed) as usize
}

pub fn crash_reports() -> bool {
    CRASH.load(Ordering::Relaxed)
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
