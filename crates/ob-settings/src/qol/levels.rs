use super::*;

/// The streamlined graphics page's Enhanced quality levels
/// (docs/developing/settings-page.md "Graphics mode and quality levels"): fixed sets
/// of the existing rows' values, the same on every GPU (no class ladder).
/// Like the presets, a level is not stored: [`Level::of`] finds the one
/// the values match, so options.txt keeps only its `enh.*` lines and a file
/// that matches no level reads "Custom".
///
/// A level sets every graphics value the presets reset except the main
/// page's own choices ([`Level::keep_choices`]): Upscaling and Render
/// Scale, Anti-Aliasing with its MSAA / filter / sharpening rows, Motion
/// Blur, Rain on the Lens and the ray-traced tiers (Modern + RT lighting,
/// Ray Traced reflections). Those stay as the player set them when a level
/// is chosen and do not count when the level is found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    /// For integrated GPUs: the cheap rows only (within Original + 3 ms at
    /// 1920×1080 on the reference integrated GPU). Keeps the original's car
    /// highlight (no light probes, no screen-space reflections:
    /// [`scene_lights_car`]).
    Low,
    /// The High preset's rows ([`Preset::Modern`] without its MSAA 8x,
    /// an Anti-Aliasing choice) plus Volumetric Fog Low.
    Medium,
    /// The Ultra preset's rows ([`Preset::Ultra`] without its SSAA 2x).
    High,
}

pub const LEVELS: [Level; 3] = [Level::Low, Level::Medium, Level::High];

impl Level {
    pub fn name(self) -> &'static str {
        match self {
            Level::Low => "Low",
            Level::Medium => "Medium",
            Level::High => "High",
        }
    }

    /// The level Graphics mode Enhanced starts on for a GPU class: Low on
    /// an integrated GPU (Medium costs 18–28 ms there at 1080p at the World
    /// Detail a fresh install picks, 3, and 44–51 ms with everything raised;
    /// docs/notes/performance/integrated-gpu.md), Medium elsewhere.
    pub fn for_class(class: GpuClass) -> Level {
        match class {
            GpuClass::Integrated => Level::Low,
            GpuClass::Mid | GpuClass::High => Level::Medium,
        }
    }

    /// `e` with this level's values; the QoL values (as the presets) and
    /// the main page's choices are kept.
    pub fn apply(self, e: &mut Enhancements) {
        let keep = *e;
        *e = e.preset_base();
        // Low (measured on the reference integrated GPU at 1080p,
        // docs/developing/settings-page.md "Level costs"): High Precision Post
        // (+2.7..3.9 ms there), Wet Roads (+2.7 ms in rain) and the 256 car
        // reflection (~+0.7 ms) start at Medium.
        e.enhanced_paint = true;
        e.enhanced_reflections = true;
        e.road_reflection = 1;
        e.shadows = 2;
        e.lod_fade = true;
        e.detail_distance = 150;
        e.wet_car = true;
        e.rain_detail = 1;
        if self != Level::Low {
            e.hdr_post = 1;
            e.car_reflection = 256;
            e.wet_roads = true;
            // Preset::Modern's rows (docs/notes/graphics/lamps-shadows.md, docs/notes/graphics/lighting2.md).
            e.ssao = 50;
            e.dynamic_lamps = true;
            e.lamp_shadows = 1;
            e.contact_shadows = true;
            e.spray = true;
            e.headlight_beams = 1;
            e.light_probes = true;
            // docs/notes/graphics/atmosphere.md "The tiers": Low at Medium, High at High.
            e.volumetric_fog = 1;
        }
        if self == Level::High {
            // Preset::Ultra's rows.
            e.hdr_post = 2;
            e.road_reflection = 2;
            e.car_reflection = 1024;
            e.detail_distance = 200;
            e.car_detail = 200;
            e.keep_loaded = true;
            e.ssr = 1;
            e.lamp_shadows = 2;
            e.volumetric_fog = 2;
            e.particles = 2;
            e.water = 1;
            e.rain_detail = 2;
            e.wet_interaction = 1;
            e.enhanced_glass = true;
        }
        Level::keep_choices(&keep, e);
    }

    /// The main page's choices of `from` put back on `e`: the values a
    /// level neither sets nor compares.
    pub fn keep_choices(from: &Enhancements, e: &mut Enhancements) {
        e.upscale = from.upscale;
        e.render_scale = from.render_scale;
        e.post_aa = from.post_aa;
        e.aa_msaa = from.aa_msaa;
        e.aa_filter = from.aa_filter;
        e.sharpen = from.sharpen;
        e.motion_blur = from.motion_blur;
        e.lens_drops = from.lens_drops;
        if from.modern_lighting == 3 {
            e.modern_lighting = 3;
            e.modern_strength = from.modern_strength;
        }
        if from.ssr == 3 {
            e.ssr = 3;
        }
    }

    /// The level `e`'s values are (the choices aside), if any.
    pub fn of(e: &Enhancements) -> Option<Level> {
        LEVELS.into_iter().find(|l| {
            let mut x = *e;
            l.apply(&mut x);
            x == *e
        })
    }
}

/// Graphics mode Original: every graphics value native (the Original
/// preset's), the QoL values aside.
pub fn graphics_original(e: &Enhancements) -> bool {
    let mut x = *e;
    Preset::Original.apply(&mut x);
    x == *e
}

/// Ray tracing on: a ray-traced tier chosen (Modern + RT lighting or Ray
/// Traced reflections; in force only where the device traces).
pub fn ray_traced(e: &Enhancements) -> bool {
    e.modern_lighting == 3 || e.ssr == 3
}

/// The main page's Ray tracing switch: on, both ray-traced tiers; off,
/// both back off, the reflections to the level's own tier when the values
/// are a level.
pub fn set_ray_traced(e: &mut Enhancements, on: bool) {
    if on {
        e.modern_lighting = 3;
        e.ssr = 3;
        return;
    }
    let level = Level::of(e);
    if e.modern_lighting == 3 {
        e.modern_lighting = 0;
        e.modern_strength = Enhancements::default().modern_strength;
    }
    if e.ssr == 3 {
        e.ssr = 0;
    }
    if let Some(l) = level {
        l.apply(e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_choice(e: &mut Enhancements) {
        e.upscale = 2;
        e.render_scale = 75;
        e.post_aa = 7;
        e.aa_msaa = 4;
        e.aa_filter = 2;
        e.sharpen = 25;
        e.motion_blur = 2;
        e.lens_drops = true;
    }

    /// Level → values → level round-trips, with and without the main
    /// page's choices and ray tracing; the levels differ from each other
    /// and from Original; Original values are no level.
    #[test]
    fn levels_round_trip() {
        assert_eq!(Level::of(&Enhancements::default()), None);
        assert!(graphics_original(&Enhancements::default()));
        for l in LEVELS {
            for choices in [false, true] {
                for rt in [false, true] {
                    let mut e = Enhancements { fov_offset: 5, hud_scale: 90, ..Enhancements::default() };
                    if choices {
                        every_choice(&mut e);
                    }
                    set_ray_traced(&mut e, rt);
                    l.apply(&mut e);
                    assert_eq!(Level::of(&e), Some(l), "{l:?} choices {choices} rt {rt}");
                    assert_eq!(ray_traced(&e), rt);
                    assert_eq!((e.fov_offset, e.hud_scale), (5, 90), "QoL kept");
                    assert!(!graphics_original(&e));
                    if choices {
                        let mut c = Enhancements::default();
                        every_choice(&mut c);
                        assert_eq!(
                            (e.upscale, e.render_scale, e.post_aa, e.aa_msaa, e.aa_filter, e.sharpen),
                            (c.upscale, c.render_scale, c.post_aa, c.aa_msaa, c.aa_filter, c.sharpen),
                            "choices kept"
                        );
                        assert!(e.lens_drops && e.motion_blur == 2);
                    }
                    // Ray tracing off gives the level's own reflections back.
                    set_ray_traced(&mut e, false);
                    assert_eq!(Level::of(&e), Some(l), "{l:?} after ray tracing off");
                    assert!(!ray_traced(&e));
                }
            }
        }
        let mut seen = Vec::new();
        for l in LEVELS {
            let mut e = Enhancements::default();
            l.apply(&mut e);
            assert!(!seen.contains(&e), "{l:?} equals an earlier level");
            seen.push(e);
        }
    }

    /// A value off every level reads Custom: one row changed from a level.
    #[test]
    fn custom_is_detected() {
        for l in LEVELS {
            let mut e = Enhancements::default();
            l.apply(&mut e);
            e.sky = 1;
            assert_eq!(Level::of(&e), None, "{l:?} + Sky");
            let mut e = Enhancements::default();
            l.apply(&mut e);
            e.modern_lighting = 2;
            assert_eq!(Level::of(&e), None, "{l:?} + Modern Lighting (not ray traced)");
        }
        // The old presets: Ultra is High with its SSAA 2x (a choice); the others are Custom.
        let mut e = Enhancements::default();
        Preset::Ultra.apply_for(GpuClass::High, &mut e);
        assert_eq!(Level::of(&e), Some(Level::High));
        assert_eq!(e.post_aa, 6);
        for p in [Preset::Clean, Preset::Modern, Preset::Extreme] {
            let mut e = Enhancements::default();
            p.apply_for(GpuClass::High, &mut e);
            assert_eq!(Level::of(&e), None, "{p:?}");
        }
    }

    /// The car highlight rule (docs/notes/graphics/car-look.md "Car highlight from the scene, not the camera rig"): Low keeps the
    /// original's highlight, Medium, High and ray tracing light the car by
    /// the scene.
    #[test]
    fn levels_follow_the_car_highlight_rule() {
        for l in LEVELS {
            for rt in [false, true] {
                let mut e = Enhancements::default();
                set_ray_traced(&mut e, rt);
                l.apply(&mut e);
                let rows = CarShadingRows {
                    modern_lighting: e.modern_lighting,
                    ssr: e.ssr,
                    enhanced_paint: e.enhanced_paint,
                    enhanced_reflections: e.enhanced_reflections,
                    light_probes: e.light_probes,
                };
                assert_eq!(scene_lights_car(rows), rt || l != Level::Low, "{l:?} rt {rt}");
            }
        }
    }
}
