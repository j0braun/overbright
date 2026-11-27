use super::*;

/// The Graphics page's presets (docs/notes/graphics/graphics-settings-audit.md):
/// each sets every graphics value of [`Enhancements`] (the QoL values —
/// FOV, HUD scale, frame cap, saves, crash reports, HD UI — are left
/// alone). The preset is not stored: [`Enhancements::preset`] finds the
/// one the values match, so options.txt keeps only its `enh.*` lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    /// Native: every graphics value off (the default).
    Original,
    /// High Precision Post, MSAA 8x with alpha-to-coverage, Enhanced Car
    /// Paint (16x anisotropic filtering is native: Customize Display's
    /// Texture Filtering at its top, as World Detail 5 sets it).
    Clean,
    /// Clean + enhanced reflections, Road Reflection Very High (half
    /// screen), Car Reflection Very High (256 cube), ambient occlusion
    /// Medium (50 %), Car Shadow Ultra, LOD fade, World Detail Very High
    /// (150 %), Wet Roads, Dynamic Lamps, Lamp Shadows Low, Contact
    /// Shadows, Headlight Beams, Light Probes. (TAA stays manual,
    /// docs/notes/graphics/temporal.md.)
    Modern,
    /// Modern + SSAA 2x, Post-Processing Ultra (full-size glow), Road
    /// Reflection Ultra (full screen), Car Reflection Ultra (1024 cube),
    /// World Detail Ultra (200 %), Car Geometry Detail Ultra (200 %), Keep
    /// All Loaded, Screen-Space Reflections High (half resolution; Ultra
    /// stays manual), Lamp Shadows High, Volumetric Fog High, Particles
    /// Ultra, Animated Water, Rain Detail High, Wet Road Interaction (Enhanced
    /// Glass left it in the tier review). On an integrated GPU some rows step down
    /// ([`Preset::apply_for`]).
    Ultra,
    /// docs/developing/settings-page.md: Ultra + Screen-Space Reflections Ultra
    /// (full resolution), Particles Cinematic (volumetric smoke), Animated
    /// Water Realistic, Wet Road Interaction High, Surface Detail Materials +
    /// Normals, Smooth Car Shapes, World Detail Extreme: the costlier rows
    /// The look review keeps (no Next-Gen tier, no motion blur / depth of field, no
    /// Modern Lighting: those stay manual). On an integrated GPU it is
    /// Ultra's integrated ladder (none of its extras measured there).
    Extreme,
}

pub const PRESETS: [Preset; 5] = [Preset::Original, Preset::Clean, Preset::Modern, Preset::Ultra, Preset::Extreme];

impl Preset {
    pub fn name(self) -> &'static str {
        match self {
            // The settings page's names (the variants keep their old ones).
            Preset::Original => "Original",
            Preset::Clean => "Balanced",
            Preset::Modern => "High",
            Preset::Ultra => "Ultra",
            Preset::Extreme => "Extreme",
        }
    }

    /// `e` with this preset's graphics values (QoL values kept), its effect
    /// tiers for this machine's GPU class ([`gpu_class`]).
    pub fn apply(self, e: &mut Enhancements) {
        self.apply_for(gpu_class(), e);
    }

    /// [`Preset::apply`] for a GPU class (docs/notes/performance/budget.md, GPU classes): High (and Mid, for
    /// now) is the preset as measured on the reference desktop GPU; Integrated steps the
    /// costliest rows down (measured on the reference integrated GPU).
    pub fn apply_for(self, class: GpuClass, e: &mut Enhancements) {
        self.apply_high(e);
        if self == Preset::Original || self == Preset::Clean {
            return;
        }
        let ultra = matches!(self, Preset::Ultra | Preset::Extreme);
        match class {
            GpuClass::High => {}
            // No step without a measured mid-class card: on the reference desktop GPU
            // Ultra with MSAA 8x instead of SSAA 2x was 0.5–0.9 ms *slower* at
            // 4K (docs/notes/performance/budget.md, GPU classes), so the obvious candidate is no
            // saving. The class is kept for when one is measured.
            GpuClass::Mid => {}
            GpuClass::Integrated => {
                // Measured on the reference integrated GPU at 1080p (docs/notes/performance/budget.md, GPU classes;
                // World Detail 5, the bench default with 8x MSAA): Modern there is 28–50 ms. Headlight Beams stay On (Off is 2.3 ms
                // slower: the cars' lamps go to the world pass) and Rain Detail On
                // (+0.7 ms while it rains); Rain Detail High +1.3 more,
                // Volumetric Fog Low +2.0 / High +2.7 ms (docs/notes/graphics/atmosphere.md "The tiers")
                // for a subtle look, so off.
                e.rain_detail = 1;
                e.volumetric_fog = 0;
                if ultra {
                    // SSAA 2x doubles the fill (Ultra 128 ms vs Modern 34 ms):
                    // Modern's MSAA 8x; the half-size glow; the native smoke
                    // (Particles High still drew +21 ms in the burnout there); the
                    // water and wet-road extras off (several ms each when in view).
                    e.post_aa = 5;
                    e.hdr_post = 1;
                    e.particles = 0;
                    e.water = 0;
                    e.wet_interaction = 0;
                    // Enhanced Glass +3.9 ms at a race start there (the 8x
                    // resolve of the scene copy: pass 7.8 ms).
                    e.enhanced_glass = false;
                }
                if self == Preset::Extreme {
                    // Extreme's extras are not measured on an integrated
                    // GPU: Ultra's integrated ladder.
                    e.ssr = 1;
                    e.surface_detail = 0;
                    e.smooth_cars = false;
                    e.far_draw = false;
                }
            }
        }
    }

    fn apply_high(self, e: &mut Enhancements) {
        // The QoL values kept (`presets keep` in the option table).
        *e = e.preset_base();
        if self == Preset::Original {
            return;
        }
        // Clean.
        e.hdr_post = 1;
        e.post_aa = 5;
        e.enhanced_paint = true;
        if self == Preset::Clean {
            return;
        }
        // Modern.
        e.enhanced_reflections = true;
        e.road_reflection = 1;
        e.car_reflection = 256;
        e.ssao = 50;
        e.shadows = 2;
        e.lod_fade = true;
        e.detail_distance = 150;
        e.wet_roads = true;
        // See docs/notes/graphics/lamps-shadows.md.
        e.dynamic_lamps = true;
        e.lamp_shadows = 1;
        e.contact_shadows = true;
        // docs/notes/graphics/weather.md: only while it rains; cheap.
        e.wet_car = true;
        e.rain_detail = 1;
        e.spray = true;
        // docs/notes/graphics/lighting2.md: beams and probes by their cost (Low / CPU).
        e.headlight_beams = 1;
        e.light_probes = true;
        if self == Preset::Modern {
            return;
        }
        // Ultra (SSR Ultra, full resolution, stays manual).
        e.post_aa = 6;
        e.hdr_post = 2;
        e.road_reflection = 2;
        e.car_reflection = 1024;
        e.detail_distance = 200;
        e.car_detail = 200;
        e.keep_loaded = true;
        e.ssr = 1;
        e.lamp_shadows = 2;
        // docs/notes/graphics/temporal.md: TAA and Upscaling stay manual.
        e.volumetric_fog = 2;
        // docs/notes/graphics/smoke.md "Particles High and Ultra: GPU particles and detailed smoke": Ultra by its measured cost.
        e.particles = 2;
        // docs/notes/graphics/water.md: Animated Water by its measured cost.
        e.water = 1;
        // Twice the drops and splashes; Lens Drops stays manual (a
        // camera effect, taste).
        e.rain_detail = 2;
        // docs/notes/graphics/wet-road-car.md: only while the road is wet.
        e.wet_interaction = 1;
        // Enhanced Glass, back in Ultra (docs/notes/performance/budget.md, the final effects pass):
        // the draws car.efx blends over a pane (the NFSPEED rear-window
        // decal) are drawn after the glass composite again, and the scene copy
        // covers only the glass's screen rect: +0.11 / +0.35 ms at 4K (racing /
        // race start; 0.65 before); the look rules met.
        e.enhanced_glass = true;
        if self == Preset::Ultra {
            return;
        }
        // Extreme (docs/developing/settings-page.md): the costlier tiers the look
        // review keeps.
        e.ssr = 2;
        e.particles = 3;
        e.water = 2;
        e.wet_interaction = 2;
        e.surface_detail = 2;
        e.smooth_cars = true;
        e.far_draw = true;
    }
}

/// docs/notes/performance/budget.md, GPU classes: the GPU class the presets' effect tiers
/// follow. Only [`Preset::apply`] reads it: a value the player picked is never
/// changed, and nothing switches by itself while playing (no automatic
/// dynamic resolution).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuClass {
    /// Integrated or software adapters (measured: the reference integrated GPU, 2 CUs).
    Integrated,
    /// Discrete GPUs with under 11.5 GiB of video memory (8 / 10 GB cards).
    /// **Inferred**: memory as the class's proxy, no such card measured; its
    /// presets are High's until one is.
    Mid,
    /// Discrete GPUs with 11.5 GiB or more, or of unknown memory (the
    /// reference desktop GPU, 16 GB, measured). The presets as before the GPU classes.
    High,
}

impl GpuClass {
    pub fn name(self) -> &'static str {
        match self {
            GpuClass::Integrated => "integrated",
            GpuClass::Mid => "mid",
            GpuClass::High => "high",
        }
    }
}

/// The class of an adapter: its type and video memory (bytes, 0 = unknown;
/// `upscale_pack::vram_bytes`, the DRM sysfs node).
pub fn classify_gpu(device_type: wgpu::DeviceType, vram: u64) -> GpuClass {
    match device_type {
        wgpu::DeviceType::IntegratedGpu | wgpu::DeviceType::Cpu => GpuClass::Integrated,
        _ if vram != 0 && vram < (23u64 << 30) / 2 => GpuClass::Mid,
        _ => GpuClass::High,
    }
}
