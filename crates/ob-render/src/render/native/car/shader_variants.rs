//! The car shader modules : every form of
//! car.wgsl the renderer builds — the 32 slots by [`module_index`]
//! (Enhanced Car Paint, Wet Car, Surface Detail, the temporal pass's LOD
//! bias, Modern Lighting), Enhanced Glass's module and the SSR mask's — in
//! one builder.
//!
//! car.wgsl holds no composer directives (`core::shader`; its `.wgsl`
//! files are split), so the forms are the same string surgery on fixed lines
//! as before. A hook line that is not there exactly once is a
//! [`VariantError`] (the old `assert!` message), and [`startup_check`]
//! builds every form when the cars' renderer is made, so a broken form is
//! reported at start, not at its first draw.
//!
//! Transitional place: this file is to move to `render/car/shader_variants.rs`.

use super::CarMode;

/// A car module form whose hook line is missing or not unique: the message
/// the old `assert!` gave.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VariantError(pub &'static str);

impl std::fmt::Display for VariantError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for VariantError {}

/// `Err(what)` unless `ok` (the old `assert!(ok, what)`).
fn ensure(ok: bool, what: &'static str) -> Result<(), VariantError> {
    if ok { Ok(()) } else { Err(VariantError(what)) }
}

/// One car module slot: [`module_index`]'s bits as fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CarKey {
    pub paint: bool,
    pub wet: bool,
    pub surface: bool,
    pub biased: bool,
    pub modern: bool,
}

impl CarKey {
    /// The number of slots (`CarShared::modules`).
    pub const SLOTS: usize = 32;

    /// The slot of a car pipeline set's mode.
    pub(crate) fn of(mode: CarMode) -> CarKey {
        CarKey {
            paint: mode.1,
            wet: mode.2,
            surface: mode.3,
            biased: mode.0.bias_q != 0,
            modern: mode.0.modern != 0,
        }
    }

    /// The slot's index ([`module_index`]'s layout).
    pub fn index(self) -> usize {
        usize::from(self.paint)
            | usize::from(self.wet) << 1
            | usize::from(self.surface) << 2
            | usize::from(self.biased) << 3
            | usize::from(self.modern) << 4
    }

    /// The slot at `index` (< [`CarKey::SLOTS`]).
    pub fn from_index(index: usize) -> CarKey {
        let bit = |b: usize| index >> b & 1 != 0;
        CarKey { paint: bit(0), wet: bit(1), surface: bit(2), biased: bit(3), modern: bit(4) }
    }

    /// Every slot, by index.
    pub fn all() -> impl Iterator<Item = CarKey> {
        (0..CarKey::SLOTS).map(CarKey::from_index)
    }
}

/// A mode's module slot in [`CarShared`] (0: the native module).
pub(crate) fn module_index(mode: CarMode) -> usize {
    usize::from(mode.1)
        | usize::from(mode.2) << 1
        | usize::from(mode.3) << 2
        | usize::from(mode.0.bias_q != 0) << 3
        | usize::from(mode.0.modern != 0) << 4
}

/// The module source of slot `key` (slot 0 is car.wgsl as it is).
pub(crate) fn module_source(key: CarKey) -> Result<String, VariantError> {
    let source = car_source_n(key.paint, key.wet, key.surface)?;
    // Modern Lighting's form (the native car.wgsl never holds it).
    let source = if key.modern { modern_car_source(&source)? } else { source };
    Ok(if key.biased { crate::render::temporal::biased_source(&source) } else { source })
}

/// Enhanced Glass's module for (Enhanced Paint, Wet Car, the temporal
/// pass's LOD bias).
pub(crate) fn glass_module_source(paint: bool, wet: bool, biased: bool) -> Result<String, VariantError> {
    let source = glass_source(paint, wet)?;
    Ok(if biased { crate::render::temporal::biased_source(&source) } else { source })
}

/// The draws' table as car.wgsl declares it ([`immediate_form`]'s hook).
const DRAWS_DECL: &str = "@group(1) @binding(0) var<storage, read> draws: array<Draw>;\n";
/// The entry reads [`immediate_form`] turns into `draws[car_draw]`: the
/// vertex stage's (by its instance index) and the fragment stages' (by
/// the flat interpolant the vertex stage passes on).
const DRAW_READS: [&str; 2] = ["draws[draw_index]", "draws[in.draw_index]"];

/// The draw index (docs/notes/performance/car-draw-index.md): a car module form with the draw's
/// entry index in an immediate (`car_draw`, 4 bytes, set before each draw)
/// instead of the instance index / flat interpolant. The value is the
/// same entry; the compiler then knows it is one per draw, so the 560-byte
/// record is read with scalar loads once per wave instead of vector loads
/// per vertex and per pixel. Used when the device has
/// `wgpu::Features::IMMEDIATES`; without it the forms above are used as
/// they are. Every `draws[…]` read of the form is replaced (one hook
/// missing is an error).
pub(crate) fn immediate_form(source: &str) -> Result<String, VariantError> {
    ensure(source.matches(DRAWS_DECL).count() == 1, "car.wgsl's draws table")?;
    ensure(source.matches(DRAW_READS[0]).count() == 1, "vs_main's draw read")?;
    ensure(source.matches(DRAW_READS[1]).count() >= 1, "the fragment stages' draw reads")?;
    let out = source
        .replace(DRAWS_DECL, &format!("{DRAWS_DECL}// The draw's entry (one per draw, read as a scalar).\nvar<immediate> car_draw: u32;\n"))
        .replace(DRAW_READS[0], "draws[car_draw]")
        .replace(DRAW_READS[1], "draws[car_draw]");
    ensure(out.matches("draws[").count() == out.matches("draws[car_draw]").count(), "a draw read by another index")?;
    Ok(out)
}

/// `source` in its [`immediate_form`] when `immediates`.
fn with_immediates(source: Result<String, VariantError>, immediates: bool) -> Result<String, VariantError> {
    if immediates { immediate_form(&source?) } else { source }
}

/// [`module_source`] for a device with (`immediates`) or without
/// immediates.
pub(crate) fn module_source_for(key: CarKey, immediates: bool) -> Result<String, VariantError> {
    with_immediates(module_source(key), immediates)
}

/// [`glass_module_source`] for a device with or without immediates.
pub(crate) fn glass_module_source_for(
    paint: bool,
    wet: bool,
    biased: bool,
    immediates: bool,
) -> Result<String, VariantError> {
    with_immediates(glass_module_source(paint, wet, biased), immediates)
}

/// [`ssr_mask_source`] for a device with or without immediates.
pub(crate) fn ssr_mask_source_for(surface: bool, immediates: bool) -> Result<String, VariantError> {
    with_immediates(ssr_mask_source(surface), immediates)
}

/// Every form above built once: the 32 slots, the glass modules, the SSR
/// mask's, each with and without immediates.
pub(crate) fn check_all() -> Result<(), VariantError> {
    for immediates in [false, true] {
        for key in CarKey::all() {
            module_source_for(key, immediates)?;
        }
        for i in 0..8 {
            glass_module_source_for(i & 1 != 0, i & 2 != 0, i & 4 != 0, immediates)?;
        }
        for surface in [false, true] {
            ssr_mask_source_for(surface, immediates)?;
        }
    }
    Ok(())
}

/// [`check_all`] once per process (the cars' renderer calls it when made):
/// a broken form panics here with the old `assert!` message.
pub(crate) fn startup_check() {
    static CHECKED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    CHECKED.get_or_init(|| check_all().unwrap_or_else(|e| panic!("{e}")));
}

/// A form [`startup_check`] has built: its source.
pub(crate) fn built(source: Result<String, VariantError>) -> String {
    source.unwrap_or_else(|e| panic!("{e}"))
}

/// Enhanced Car Paint's WGSL: car.wgsl with its final combine replaced by
/// car_paint.wgsl's `paint_layer`, which returns the native combine for
/// draws without a paint type.
pub(crate) fn paint_source() -> Result<String, VariantError> {
    const COMBINE: &str = "    r0 = 2.0 * r0 * r1.w + r1.xyz;\n";
    let native = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car.wgsl"));
    ensure(native.matches(COMBINE).count() == 1, "car.wgsl's final combine")?;
    Ok(native.replace(COMBINE, "    r0 = paint_layer(in, r0, env, texel.rgb, specular, r1, direction);\n")
        + include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car_paint.wgsl")))
}

/// Screen-Space Reflections' car mask WGSL: car.wgsl (its vertex stage and
/// bindings) with car_ssr.wgsl's fragment stages appended.
/// Wet Car's WGSL: car.wgsl (or Enhanced Car Paint's) with the final
/// combine's result handed to car_wet.wgsl's `wet_layer`.
pub(crate) fn wet_source(paint: bool) -> Result<String, VariantError> {
    let (source, line) = combine_source(paint)?;
    ensure(source.matches(line).count() == 1, "the final combine")?;
    let expr = line.trim().trim_start_matches("r0 = ").trim_end_matches(';');
    Ok(source.replace(line, &format!("    r0 = wet_layer(in, {expr}, r1, direction);\n"))
        + include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car_wet.wgsl")))
}

/// car.wgsl or Enhanced Car Paint's form, and its final combine line.
fn combine_source(paint: bool) -> Result<(String, &'static str), VariantError> {
    const COMBINE: &str = "    r0 = 2.0 * r0 * r1.w + r1.xyz;\n";
    const PAINT: &str = "    r0 = paint_layer(in, r0, env, texel.rgb, specular, r1, direction);\n";
    Ok(if paint {
        (paint_source()?, PAINT)
    } else {
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car.wgsl")).to_string(), COMBINE)
    })
}

/// The module source for (Enhanced Car Paint, Wet Car).
pub(crate) fn car_source(paint: bool, wet: bool) -> Result<String, VariantError> {
    if wet { wet_source(paint) } else { Ok(combine_source(paint)?.0) }
}

/// Surface Detail's hooks in car_colour (car_surface.wgsl): the relief
/// taken before the env cube's sample turns its direction and weights the
/// specular, and the diffuse.
pub(crate) const SURFACE_ENV: &str = "    let env = textureSample(envmap, envmap_sampler, direction).rgb;\n";
const SURFACE_DIFFUSE: &str = "    var r1 = 2.0 * texel * in.diffuse;\n";

/// `source` (a car module's) with Surface Detail's hooks and
/// car_surface.wgsl appended.
fn surface_source(source: &str) -> Result<String, VariantError> {
    ensure(source.matches(SURFACE_ENV).count() == 1, "car_colour's env sample")?;
    ensure(source.matches(SURFACE_DIFFUSE).count() == 1, "car_colour's diffuse")?;
    Ok(source
        .replace(SURFACE_ENV, &format!("    let relief = surface_relief(in);\n    direction += relief.turn;\n    specular = vec4<f32>(specular.xyz * relief.specular, specular.w);\n{SURFACE_ENV}"))
        .replace(SURFACE_DIFFUSE, "    var r1 = 2.0 * texel * vec4<f32>(in.diffuse.xyz * relief.diffuse, in.diffuse.w);\n")
        + include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car_surface.wgsl")))
}

/// [`car_source_n`] for other modules' tests.
#[cfg(test)]
pub(crate) fn source_for_tests(paint: bool, wet: bool, surface: bool) -> String {
    built(car_source_n(paint, wet, surface))
}

/// The module source for (Enhanced Car Paint, Wet Car, Surface Detail).
pub(crate) fn car_source_n(paint: bool, wet: bool, surface: bool) -> Result<String, VariantError> {
    let source = car_source(paint, wet)?;
    if surface { surface_source(&source) } else { Ok(source) }
}

/// The SSR mask's module; with Surface Detail its normal takes the map's
/// tilt (derivatives taken at the top, with the normal's spread).
pub(crate) fn ssr_mask_source(surface: bool) -> Result<String, VariantError> {
    let mask = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car_ssr.wgsl"));
    if !surface {
        return Ok(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car.wgsl")).to_string() + mask);
    }
    const SPREAD: &str = "    let spread = length(fwidth(nw0 * inverseSqrt(max(dot(nw0, nw0), 1e-12))));\n";
    const NORMAL: &str = "    var nw = nw0 * inverseSqrt(max(dot(nw0, nw0), 1e-12));\n";
    ensure(mask.matches(SPREAD).count() == 1 && mask.matches(NORMAL).count() == 1, "car_ssr.wgsl's normal")?;
    let mask = mask
        .replace(SPREAD, &format!("{SPREAD}    let surface_nw = surface_world_normal(in, nw0);\n"))
        .replace(NORMAL, "    var nw = surface_nw;\n");
    Ok(surface_source(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car.wgsl")))? + &mask)
}

/// The car's module source for (Enhanced Paint, Wet Car) with
/// car_glass.wgsl appended (the native modules are untouched).
pub(crate) fn glass_source(paint: bool, wet: bool) -> Result<String, VariantError> {
    Ok(car_source(paint, wet)? + include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car_glass.wgsl")))
}

/// The cars' module with Modern Lighting (`source`: car.wgsl or one of its
/// forms): the Dynamic Lamps line takes the Modern path when the draw's
/// lamps are in Modern Lighting's form (a negative `lamps[1].w`), the
/// result gets their highlight, and car_modern.wgsl is appended. The
/// native module is never touched (its text is car.wgsl as it was).
pub fn modern_car_source(source: &str) -> Result<String, VariantError> {
    const LAMPS: &str = "        r1 = vec4<f32>(r1.xyz + 2.0 * texel.rgb * car_lamps(in), r1.w);\n";
    const RETURN: &str = "    return vec4<f32>(r0, r1.w);\n}";
    ensure(source.matches(LAMPS).count() == 1, "car_colour's Dynamic Lamps line")?;
    ensure(source.matches(RETURN).count() == 1, "car_colour's return")?;
    Ok(source
        .replace(LAMPS, "        if draw.lamps[1].w < 0.0 {\n            let m = car_modern(in);\n            r1 = vec4<f32>(r1.xyz * car_modern_ambient() + 2.0 * texel.rgb * m.diffuse, r1.w);\n            g_car_modern_spec = m.spec;\n        } else {\n    LAMPS_LINE        }\n".replace("LAMPS_LINE", LAMPS).as_str())
        .replace(RETURN, "    return vec4<f32>(r0 + g_car_modern_spec, r1.w);\n}")
        + include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car_modern.wgsl")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wgpu::naga;

    /// The assembly before this builder, verbatim (car.rs, car_glass.rs,
    /// modern_light_bold.rs; `assert!` on a missing hook): the oracle of the
    /// text-identity test (to be deleted with the world's).
    #[allow(dead_code)]
    mod oracle {
        /// Enhanced Car Paint's WGSL: car.wgsl with its final combine replaced by
        /// car_paint.wgsl's `paint_layer`, which returns the native combine for
        /// draws without a paint type.
        fn paint_source() -> String {
            const COMBINE: &str = "    r0 = 2.0 * r0 * r1.w + r1.xyz;\n";
            let native = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car.wgsl"));
            assert_eq!(native.matches(COMBINE).count(), 1, "car.wgsl's final combine");
            native.replace(COMBINE, "    r0 = paint_layer(in, r0, env, texel.rgb, specular, r1, direction);\n")
                + include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car_paint.wgsl"))
        }

        /// Screen-Space Reflections' car mask WGSL: car.wgsl (its vertex stage and
        /// bindings) with car_ssr.wgsl's fragment stages appended.
        /// Wet Car's WGSL: car.wgsl (or Enhanced Car Paint's) with the final
        /// combine's result handed to car_wet.wgsl's `wet_layer`.
        fn wet_source(paint: bool) -> String {
            let (source, line) = combine_source(paint);
            assert_eq!(source.matches(line.as_str()).count(), 1, "the final combine");
            let expr = line.trim().trim_start_matches("r0 = ").trim_end_matches(';');
            source.replace(line.as_str(), &format!("    r0 = wet_layer(in, {expr}, r1, direction);\n"))
                + include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car_wet.wgsl"))
        }

        /// car.wgsl or Enhanced Car Paint's form, and its final combine line.
        fn combine_source(paint: bool) -> (String, String) {
            const COMBINE: &str = "    r0 = 2.0 * r0 * r1.w + r1.xyz;\n";
            const PAINT: &str = "    r0 = paint_layer(in, r0, env, texel.rgb, specular, r1, direction);\n";
            let (source, line) = if paint {
                (paint_source(), PAINT)
            } else {
                (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car.wgsl")).to_string(), COMBINE)
            };
            (source, line.to_string())
        }

        /// The module source for (Enhanced Car Paint, Wet Car).
        fn car_source(paint: bool, wet: bool) -> String {
            if wet { wet_source(paint) } else { combine_source(paint).0 }
        }

        /// Surface Detail's hooks in car_colour (car_surface.wgsl): the relief
        /// taken before the env cube's sample turns its direction and weights the
        /// specular, and the diffuse.
        const SURFACE_ENV: &str = "    let env = textureSample(envmap, envmap_sampler, direction).rgb;\n";
        const SURFACE_DIFFUSE: &str = "    var r1 = 2.0 * texel * in.diffuse;\n";

        /// `source` (a car module's) with Surface Detail's hooks and
        /// car_surface.wgsl appended.
        fn surface_source(source: &str) -> String {
            assert_eq!(source.matches(SURFACE_ENV).count(), 1, "car_colour's env sample");
            assert_eq!(source.matches(SURFACE_DIFFUSE).count(), 1, "car_colour's diffuse");
            source
        .replace(SURFACE_ENV, &format!("    let relief = surface_relief(in);\n    direction += relief.turn;\n    specular = vec4<f32>(specular.xyz * relief.specular, specular.w);\n{SURFACE_ENV}"))
        .replace(SURFACE_DIFFUSE, "    var r1 = 2.0 * texel * vec4<f32>(in.diffuse.xyz * relief.diffuse, in.diffuse.w);\n")
        + include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car_surface.wgsl"))
        }

        /// [`car_source_n`] for other modules' tests.
        pub(crate) fn source_for_tests(paint: bool, wet: bool, surface: bool) -> String {
            car_source_n(paint, wet, surface)
        }

        /// The module source for (Enhanced Car Paint, Wet Car, Surface Detail).
        fn car_source_n(paint: bool, wet: bool, surface: bool) -> String {
            let source = car_source(paint, wet);
            if surface { surface_source(&source) } else { source }
        }

        /// The SSR mask's module; with Surface Detail its normal takes the map's
        /// tilt (derivatives taken at the top, with the normal's spread).
        pub(super) fn ssr_mask_source(surface: bool) -> String {
            let mask = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car_ssr.wgsl"));
            if !surface {
                return include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car.wgsl")).to_string() + mask;
            }
            const SPREAD: &str = "    let spread = length(fwidth(nw0 * inverseSqrt(max(dot(nw0, nw0), 1e-12))));\n";
            const NORMAL: &str = "    var nw = nw0 * inverseSqrt(max(dot(nw0, nw0), 1e-12));\n";
            assert!(mask.matches(SPREAD).count() == 1 && mask.matches(NORMAL).count() == 1, "car_ssr.wgsl's normal");
            let mask = mask
                .replace(SPREAD, &format!("{SPREAD}    let surface_nw = surface_world_normal(in, nw0);\n"))
                .replace(NORMAL, "    var nw = surface_nw;\n");
            surface_source(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car.wgsl"))) + &mask
        }

        /// The car's module source for (Enhanced Paint, Wet Car) with
        /// car_glass.wgsl appended (the native modules are untouched).
        pub(super) fn glass_source(paint: bool, wet: bool) -> String {
            car_source(paint, wet) + include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car_glass.wgsl"))
        }

        /// The cars' module with Modern Lighting (`source`: car.wgsl or one of its
        /// forms): the Dynamic Lamps line takes the Modern path when the draw's
        /// lamps are in Modern Lighting's form (a negative `lamps[1].w`), the
        /// result gets their highlight, and car_modern.wgsl is appended. The
        /// native module is never touched (its text is car.wgsl as it was).
        pub fn modern_car_source(source: &str) -> String {
            const LAMPS: &str = "        r1 = vec4<f32>(r1.xyz + 2.0 * texel.rgb * car_lamps(in), r1.w);\n";
            const RETURN: &str = "    return vec4<f32>(r0, r1.w);\n}";
            assert_eq!(source.matches(LAMPS).count(), 1, "car_colour's Dynamic Lamps line");
            assert_eq!(source.matches(RETURN).count(), 1, "car_colour's return");
            source
        .replace(LAMPS, "        if draw.lamps[1].w < 0.0 {\n            let m = car_modern(in);\n            r1 = vec4<f32>(r1.xyz * car_modern_ambient() + 2.0 * texel.rgb * m.diffuse, r1.w);\n            g_car_modern_spec = m.spec;\n        } else {\n    LAMPS_LINE        }\n".replace("LAMPS_LINE", LAMPS).as_str())
        .replace(RETURN, "    return vec4<f32>(r0 + g_car_modern_spec, r1.w);\n}")
        + include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car_modern.wgsl"))
        }

        /// `CarShared::module`'s body before S4.
        pub fn module(key: super::CarKey) -> String {
            let source = car_source_n(key.paint, key.wet, key.surface);
            // Modern Lighting's form (the native car.wgsl never holds it).
            let source = if key.modern { modern_car_source(&source) } else { source };
            if key.biased { crate::render::temporal::biased_source(&source) } else { source }
        }

        /// `GlassShared::pipelines`' module before S4.
        pub fn glass(paint: bool, wet: bool, biased: bool) -> String {
            let source = glass_source(paint, wet);
            if biased { crate::render::temporal::biased_source(&source) } else { source }
        }
    }

    fn validate(what: &str, src: &str) {
        let module = naga::front::wgsl::parse_str(src).unwrap_or_else(|e| panic!("{what}: {}", e.emit_to_string(src)));
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
            .validate(&module)
            .unwrap_or_else(|e| panic!("{what}: {e:?}"));
    }

    /// Text identity: every car module slot, the glass modules and the SSR
    /// mask's are byte for byte the old assembly's text, and validate.
    #[test]
    fn car_modules_match_the_old_assembly() {
        check_all().unwrap();
        let native = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car.wgsl"));
        let mut n = 0;
        for (i, key) in CarKey::all().enumerate() {
            assert_eq!(key.index(), i);
            let mode = (
                crate::render::SceneMode {
                    bias_q: i8::from(key.biased),
                    modern: u8::from(key.modern),
                    ..crate::render::SceneMode::NATIVE
                },
                key.paint,
                key.wet,
                key.surface,
            );
            assert_eq!(module_index(mode), i, "{key:?}");
            assert_eq!(CarKey::of(mode), key);
            let new = module_source(key).unwrap();
            assert!(new == oracle::module(key), "slot {i} {key:?}");
            if i == 0 {
                assert_eq!(new, native);
            }
            validate(&format!("slot {i} {key:?}"), &new);
            n += 1;
        }
        for i in 0..8 {
            let (paint, wet, biased) = (i & 1 != 0, i & 2 != 0, i & 4 != 0);
            let new = glass_module_source(paint, wet, biased).unwrap();
            assert!(new == oracle::glass(paint, wet, biased), "glass {paint} {wet} {biased}");
            validate(&format!("glass {paint} {wet} {biased}"), &new);
            n += 1;
        }
        for surface in [false, true] {
            let new = ssr_mask_source(surface).unwrap();
            assert!(new == oracle::ssr_mask_source(surface), "ssr mask {surface}");
            validate(&format!("ssr mask {surface}"), &new);
            n += 1;
        }
        assert_eq!(n, 32 + 8 + 2);
        startup_check();
    }

    /// Draw-index pin: every form's immediate form is the form with exactly
    /// the declaration added and its `draws[…]` reads re-indexed (nothing
    /// else), declares 4 bytes of immediates, and validates; slot 0's is
    /// pinned in full against car.wgsl; the fallback (no immediates) is the
    /// form itself.
    #[test]
    fn immediate_forms_change_only_the_draw_index() {
        let native = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car.wgsl"));
        let decl = "@group(1) @binding(0) var<storage, read> draws: array<Draw>;\n";
        let slot0 = native
            .replace(
                decl,
                &format!("{decl}// The draw's entry (one per draw, read as a scalar).\nvar<immediate> car_draw: u32;\n"),
            )
            .replace("draws[draw_index]", "draws[car_draw]")
            .replace("draws[in.draw_index]", "draws[car_draw]");
        assert_eq!(module_source_for(CarKey::default(), true).unwrap(), slot0);
        assert_eq!(module_source_for(CarKey::default(), false).unwrap(), native);
        // The reads it re-indexes: vs_main, car_colour, car_lamps (two).
        assert_eq!(native.matches("draws[draw_index]").count(), 1);
        assert_eq!(native.matches("draws[in.draw_index]").count(), 3);
        let mut forms: Vec<(String, String)> = Vec::new();
        for key in CarKey::all() {
            forms.push((format!("slot {key:?}"), module_source(key).unwrap()));
        }
        for i in 0..8 {
            forms.push((format!("glass {i}"), glass_module_source(i & 1 != 0, i & 2 != 0, i & 4 != 0).unwrap()));
        }
        for surface in [false, true] {
            forms.push((format!("ssr mask {surface}"), ssr_mask_source(surface).unwrap()));
        }
        for (what, old) in &forms {
            let new = immediate_form(old).unwrap();
            assert!(!new.contains("draws[draw_index]") && !new.contains("draws[in.draw_index]"), "{what}");
            // Undoing the change gives the old text exactly.
            let undone = new.replace(
                "// The draw's entry (one per draw, read as a scalar).\nvar<immediate> car_draw: u32;\n",
                "",
            );
            let n = old.matches("draws[draw_index]").count();
            assert_eq!(
                undone
                    .replacen("draws[car_draw]", "draws[draw_index]", n)
                    .replace("draws[car_draw]", "draws[in.draw_index]"),
                *old,
                "{what}"
            );
            let reads = old.matches("draws[draw_index]").count() + old.matches("draws[in.draw_index]").count();
            assert_eq!(new.matches("draws[car_draw]").count(), reads, "{what}");
            let module =
                naga::front::wgsl::parse_str(&new).unwrap_or_else(|e| panic!("{what}: {}", e.emit_to_string(&new)));
            let (_, var) = module
                .global_variables
                .iter()
                .find(|(_, v)| v.space == naga::AddressSpace::Immediate)
                .unwrap_or_else(|| panic!("{what}: no immediate"));
            assert_eq!(module.types[var.ty].inner, naga::TypeInner::Scalar(naga::Scalar::U32), "{what}");
            validate(what, &new);
        }
        assert_eq!(forms.len(), 32 + 8 + 2);
    }

    /// A missing hook is an error with the old `assert!` message, not a panic.
    #[test]
    fn a_missing_hook_is_an_error() {
        assert_eq!(immediate_form("fn x() {}\n").unwrap_err(), VariantError("car.wgsl's draws table"));
        assert_eq!(modern_car_source("fn x() {}\n").unwrap_err().to_string(), "car_colour's Dynamic Lamps line");
        assert_eq!(surface_source("").unwrap_err(), VariantError("car_colour's env sample"));
    }
}
