//! Draws a [`Car`] with the game's car effect (see car.wgsl).

use std::collections::HashMap;

use bytemuck::Zeroable;
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use self::lighting::CarLighting;
pub(crate) use self::shader_variants::{CarKey, built, module_index, module_source_for, ssr_mask_source_for};
#[cfg(test)]
pub(crate) use self::shader_variants::{
    SURFACE_ENV, car_source, car_source_n, paint_source, source_for_tests, ssr_mask_source, wet_source,
};
use crate::car::{Car, CarMaterialConstants, WheelPose};
use crate::formats::hash::bin_hash;
use crate::formats::texture::Blend;
use crate::render::{DEPTH_FORMAT, Gpu, upload_rgba, upload_texture_for};

// Refactor: car.rs split (native/car/*), car_shader_variants.rs,
// lighting.rs (lighting, lighting_world).
mod batch;
mod cover;
mod envmap;
mod instance;
pub mod lighting;
pub mod lighting_world;
mod model;
mod paint_type;
mod record;
pub use record::warm_programs;
pub mod shader_variants;
mod shared;
pub use self::batch::*;
#[allow(unused_imports)]
pub use self::cover::*;
pub use self::envmap::*;
pub use self::instance::*;
pub use self::paint_type::*;
pub use self::record::*;
pub use self::shared::*;
// Enhanced Glass (docs/notes/graphics/lighting2.md): its pass and module.
use crate::render::enh::materials::car::glass;
pub use crate::render::enh::materials::car::glass::record_glass_batch;

/// Car vertex: what car.efx reads (position, normal, uv).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct CarVertex {
    position: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct FrameUniform {
    view_proj: [[f32; 4]; 4],
    view: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct DrawUniform {
    world: [[f32; 4]; 4],
    local_eye: [f32; 4],
    material: [[f32; 4]; 6],
    powers: [f32; 4],
    /// x: light-context factor; y: enhanced shading; z: dynamic world cube.
    harmonic_scale: [f32; 4],
    /// HarmonicCoeff of the part's light context.
    harmonic: [[f32; 4]; 10],
    /// Enhanced Car Paint (clone deviation): [`PaintType::params`] when the
    /// option is on, else zeros (native; car.wgsl skips the layer).
    paint: [f32; 4],
    /// Dynamic Lamps (clone deviation, docs/notes/graphics/lamps-shadows.md): four lamps
    /// (position + outer radius, colour + inner radius); zeros natively.
    lamps: [[f32; 4]; 8],
    /// Wet Car (clone deviation, docs/notes/graphics/weather.md): the draw's wet values
    /// (`wet_car::WetParams::uniform`); zeros natively and on a dry car.
    wet: [[f32; 4]; 3],
}

pub(crate) struct Draw {
    pub(crate) blend: Blend,
    pub(crate) alpha_test: bool,
    pub(crate) texture: usize,
    pub(crate) cover_bucket: (bool, u8),
    pub(crate) submission: usize,
    pub(crate) first_index: u32,
    pub(crate) index_count: u32,
    pub(crate) base_vertex: i32,
    pub(crate) part: usize,
    pub(crate) constants: CarMaterialConstants,
    /// Enhanced Car Paint's values for this draw's material.
    pub(crate) paint: [f32; 4],
    /// Wet Car's class of this draw's material.
    pub(crate) wet_class: crate::render::wet_car::WetClass,
    /// Enhanced Glass (car_glass.rs): 0 not glass, 1 a window, 2 a
    /// lamp cover; and the draw's centre (model space), for its order.
    pub(crate) glass: u8,
    pub(crate) centre: Vec3,
    /// A blended draw after the car's first glass draw in the native
    /// order (a window decal, the lamps' sprites…): car.efx draws it over
    /// the glass, so with the glass split off it goes after the glass too.
    pub(crate) glass_after: bool,
}

/// A car texture that may get a Surface Detail map: its index in
/// `CarRenderer::textures`, the texture and its relief regions.
struct SurfaceJob {
    texture: usize,
    t: crate::formats::texture::Texture,
    regions: Vec<crate::surface_maps::CarRegion>,
}

/// car.efx's pipelines for every blend and alpha test, into one format.
type Pipelines = HashMap<(Blend, bool), wgpu::RenderPipeline>;

/// A car pipeline set's scene mode and whether it is Enhanced Car Paint's.
/// (scene mode, Enhanced Car Paint, Wet Car: the car is wet this frame).
/// (scene mode, Enhanced Car Paint, Wet Car: the car is wet this frame,
/// Surface Detail's car maps: Materials + Normals).
pub(crate) type CarMode = (crate::render::SceneMode, bool, bool, bool);

trait CarModeExt {
    const NATIVE: CarMode;
    /// The scene at `samples` with the Graphics page's values in force
    /// (`opts`, the frame's).
    fn current(samples: u32, wet: bool, opts: &crate::render::RenderOptions) -> CarMode;
    /// The cars' form of the scene's `scene` mode.
    fn of_scene(scene: crate::render::SceneMode, wet: bool, opts: &crate::render::RenderOptions) -> CarMode;
}

impl CarModeExt for CarMode {
    const NATIVE: CarMode = (crate::render::SceneMode::NATIVE, false, false, false);
    fn current(samples: u32, wet: bool, opts: &crate::render::RenderOptions) -> CarMode {
        Self::of_scene(crate::render::SceneMode::current(opts, samples), wet, opts)
    }

    fn of_scene(scene: crate::render::SceneMode, wet: bool, opts: &crate::render::RenderOptions) -> CarMode {
        // Modern Lighting's car form (1 whatever the tier: the cars take
        // no rays).
        let modern = crate::render::modern_light_bold::scene_modern(opts).min(1);
        // The cars' pipelines never take the lamps' module,
        // so its feature set is no part of their key (a new combination of
        // lamp rows built the same car pipelines again, ~7 ms).
        (
            crate::render::SceneMode {
                lamps: false,
                lamp_features: 0,
                modern,
                ..scene
            },
            opts.enhanced_paint,
            wet,
            opts.surface_detail >= 2,
        )
    }
}

const DRAW_STRIDE: u64 = 560;

/// The LOD a picked value shows (`Option<Option<u8>>`'s inner value).
trait FlattenLod {
    fn flatten_lod(self) -> Option<u8>;
}

impl FlattenLod for Option<Option<u8>> {
    fn flatten_lod(self) -> Option<u8> {
        self.flatten()
    }
}

fn frame_uniform(view_proj: Mat4, view: Mat4) -> FrameUniform {
    FrameUniform { view_proj: view_proj.to_cols_array_2d(), view: view.to_cols_array_2d() }
}

#[cfg(test)]
mod cover_order_tests {
    use super::*;

    #[test]
    fn cover_queue_interleaves_shadow_by_native_bucket_not_blend() {
        // First-use texture groups, stable draws within each group;
        // car effect 5 precedes shadow effect 13 at order 2.
        let input = [(false, 3, 1), (false, 0, 2), (true, 0, 3), (false, 2, 4), (false, 0, 2), (false, 0, 5)];
        let (before, after) = cover_draw_order(&input.map(Some));
        assert_eq!(before, vec![1, 4, 5, 3]);
        assert_eq!(after, vec![0, 2]);
    }
}

#[cfg(test)]
mod cover_group_tests {
    use super::cover_draw_order;
    #[test]
    fn texture_first_use_is_per_bucket_after_visibility() {
        // A first appears in order 3. Its excluded LOD occurrence in
        // order 0 must not register ahead of visible B, then visible A.
        let input = [Some((false, 3, 1)), None, Some((false, 0, 2)), Some((false, 0, 1)), Some((false, 0, 2))];
        assert_eq!(cover_draw_order(&input), (vec![2, 4, 3], vec![0]));
    }
}

#[cfg(test)]
mod cover_placement_tests {
    use super::*;

    #[test]
    #[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
    fn mesh_world_matches_paired_native_draw_and_live_base() {
        // Little-endian f32 columns, base then draw; paired-matrix.json
        // preserves the reconstruction and unrounded source values.
        let bytes =
            &crate::testkit::private("docs/evidence/cover-placement/paired-matrix.bin");
        let matrix = |offset: usize| {
            Mat4::from_cols_array(&std::array::from_fn(|i| {
                f32::from_le_bytes(bytes[offset + i * 4..offset + i * 4 + 4].try_into().unwrap())
            }))
        };
        let base = matrix(0);
        let expected = matrix(64);
        let actual = mesh_world(base);
        // Check RX8-sized box corners as well as the origin.
        for p in [Vec3::ZERO, Vec3::new(-2.3, -1.0, 0.0), Vec3::new(2.3, 1.0, 1.25)] {
            assert!(
                (actual.transform_point3(p) - expected.transform_point3(p)).abs().max_element() < 0.00003,
                "native draw mismatch at {p:?}: {:?}",
                actual.transform_point3(p) - expected.transform_point3(p)
            );
        }
    }
}

#[cfg(test)]
mod mesh_height_tests {
    use super::*;
    #[test]
    #[ignore = "requires original assets and a GPU"]
    fn native_mesh_height_all_parts_lods_and_batch() {
        let gpu = pollster::block_on(Gpu::new(None, Gpu::new_instance())).unwrap();
        let camera = crate::render::Camera::new(Vec3::new(2.0, -4.0, 2.0), 0.0, 0.0);
        let wheels = [WheelPose { steer: 0.3, roll: 0.4, spinner: 0.4, lift: 0.02, ..Default::default() }; 4];
        let mut checked = 0;
        for name in ["RX8", "TAXI"] {
            let car = Car::load(&crate::game_root(), name).unwrap();
            let mut renderer = CarRenderer::new(&gpu, &car, wgpu::TextureFormat::Rgba8Unorm, &crate::render::RenderOptions::in_force());
            let animated_part = renderer.draws.iter().find(|d| renderer.part_wheels[d.part].is_none()).unwrap().part;
            // Isolate one synthetic opened-part consumer from shared native channels.
            renderer.part_anim.fill(None);
            renderer.part_anim[animated_part] = Some((0, 0, Vec3::ZERO));
            renderer.poses[0][0] = Some(crate::part_anim::ChannelPose {
                rotation: glam::Quat::from_rotation_z(0.25),
                moved: Vec3::new(0.1, 0.2, 0.3),
                rest: Vec3::ZERO,
                value: 0.5,
            });
            for world in [
                Mat4::from_translation(Vec3::new(0.0, 5.4, 0.29)),
                Mat4::from_rotation_x(0.4),
                Mat4::from_scale(Vec3::new(1.0, 1.0, -1.0)),
            ] {
                let lighting = CarLighting::frontend(&crate::render::lighting::DEFAULT_RIG, &camera, world);
                for lod in 0..4 {
                    renderer.set_lod(Some(lod));
                    let mut uniforms = Vec::new();
                    renderer.draw_uniforms_at(lod, [0.0, 0.0], &lighting, world, &wheels, &mut uniforms, &crate::render::RenderOptions::in_force());
                    for (d, u) in renderer.draws.iter().zip(&uniforms) {
                        if renderer.part_lods[d.part].is_some_and(|m| m >> lod & 1 == 0) {
                            continue;
                        }
                        let animation = if d.part == animated_part {
                            Mat4::from_translation(Vec3::new(0.1, 0.2, 0.3)) * Mat4::from_rotation_z(0.25)
                        } else {
                            Mat4::IDENTITY
                        };
                        let wheel = renderer.part_wheels[d.part].map_or(Mat4::IDENTITY, |(i, rolls)| {
                            let c = renderer.wheel_centres[i];
                            let pivot = renderer.part_pivots[d.part].map_or(c, |y| Vec3::new(c.x, y, c.z));
                            wheels[i].matrix(pivot, rolls)
                        });
                        let mut copied = world;
                        copied.w_axis.z += 0.01;
                        let posed =
                            if d.part == animated_part { world * animation * world.inverse() * copied } else { copied };
                        let expected = posed * wheel * renderer.part_transforms[d.part];
                        let actual = Mat4::from_cols_array_2d(&u.world);
                        assert!(
                            actual.abs_diff_eq(expected, 0.000003),
                            "{name} lod{lod} part{} actual{actual:?} expected{expected:?}",
                            d.part
                        );
                        let (h, e) = if renderer.part_wheels[d.part].is_some() {
                            lighting.in_axes(expected)
                        } else {
                            (lighting.harmonic, lighting.local_eye)
                        };
                        assert!(
                            u.harmonic
                                .iter()
                                .flatten()
                                .zip(h.iter().flat_map(|v| v.to_array()))
                                .all(|(a, b)| (*a - b).abs() < 0.00001)
                        );
                        assert!((Vec3::from_array(u.local_eye[..3].try_into().unwrap()) - e).length() < 0.00001);
                        checked += 1;
                    }
                    let mut batch = CarBatch::new(&gpu);
                    renderer.queue(&gpu, &mut batch, Mat4::IDENTITY, Mat4::IDENTITY, &lighting, world, &wheels, &crate::render::RenderOptions::in_force());
                    assert_eq!(bytemuck::cast_slice::<_, u8>(&uniforms), bytemuck::cast_slice::<_, u8>(&batch.draws));
                }
            }
        }
        assert!(checked > 100);
        eprintln!("native mesh height checked {checked} part draws; batch identical");
    }
}

#[cfg(test)]
mod open_part_matrix_tests {
    use super::*;
    #[test]
    #[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
    fn animated_mesh_matches_original_instruction_oracle() {
        let bytes = &crate::testkit::private("docs/evidence/fe-open-part/cases.bin");
        for (case, b) in bytes.chunks_exact(192).enumerate() {
            let matrix = |offset: usize| {
                Mat4::from_cols_array(&std::array::from_fn(|i| {
                    f32::from_le_bytes(b[offset + i * 4..offset + i * 4 + 4].try_into().unwrap())
                }))
            };
            let actual = animated_mesh_world(matrix(0), matrix(64));
            assert!(actual.abs_diff_eq(matrix(128), 0.000002), "case{case}: actual{actual:?}, native{:?}", matrix(128));
        }
    }
}

#[cfg(test)]
mod paint_tests {
    use super::*;

    /// Wet Car (docs/notes/graphics/weather.md): its modules only route the final
    /// combine's result (native or Enhanced Paint's) through `wet_layer`;
    /// the native and paint modules have none of it, and a dry car is drawn
    /// with them (`wet_car::WetState::step` None → `CarMode` without wet).
    #[test]
    fn wet_module_wraps_the_final_combine() {
        let native = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car.wgsl"));
        assert!(!native.contains("wet_layer") && !paint_source().unwrap().contains("wet_layer"));
        let wet = wet_source(false).unwrap();
        assert_eq!(wet.matches("    r0 = wet_layer(in, 2.0 * r0 * r1.w + r1.xyz, r1, direction);\n").count(), 1);
        assert!(wet.ends_with(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car_wet.wgsl"))));
        let both = wet_source(true).unwrap();
        assert_eq!(
            both.matches(
                "    r0 = wet_layer(in, paint_layer(in, r0, env, texel.rgb, specular, r1, direction), r1, direction);\n"
            )
            .count(),
            1
        );
        assert!(
            both.contains("fn enhanced_paint(")
                && both.ends_with(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car_wet.wgsl")))
        );
        assert_eq!(car_source(false, false).unwrap(), native);
        let dry = <CarMode as CarModeExt>::current(1, false, &crate::render::RenderOptions::in_force());
        assert!(!dry.2);
    }

    /// The per-draw entry: 560 bytes (Wet Car's 48 at the end), `paint` right after HarmonicCoeff at
    /// byte 368, matching car.wgsl's `Draw` (vec4 alignment, no implicit padding).
    #[test]
    fn draw_uniform_packing_matches_car_wgsl() {
        assert_eq!(std::mem::size_of::<DrawUniform>() as u64, DRAW_STRIDE);
        assert_eq!(std::mem::offset_of!(DrawUniform, harmonic_scale), 192);
        assert_eq!(std::mem::offset_of!(DrawUniform, harmonic), 208);
        assert_eq!(std::mem::offset_of!(DrawUniform, paint), 368);
        assert_eq!(std::mem::offset_of!(DrawUniform, lamps), 384);
        assert_eq!(std::mem::offset_of!(DrawUniform, wet), 512);
        let wgsl = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car.wgsl"));
        let draw = &wgsl[wgsl.find("struct Draw {").unwrap()..];
        let draw = &draw[..draw.find("};").unwrap()];
        let fields: Vec<&str> = draw
            .lines()
            .skip(1)
            .map(|l| l.trim())
            .filter(|l| !l.is_empty() && !l.starts_with("//"))
            .map(|l| l.split("//").next().unwrap().trim())
            .collect();
        assert_eq!(
            fields,
            [
                "world: mat4x4<f32>,",
                "local_eye: vec4<f32>,",
                "diffuse_min: vec4<f32>,",
                "diffuse_range: vec4<f32>,",
                "specular_min: vec4<f32>,",
                "specular_range: vec4<f32>,",
                "envmap_min: vec4<f32>,",
                "envmap_range: vec4<f32>,",
                "powers: vec4<f32>,",
                "harmonic_scale: vec4<f32>,",
                "harmonic: array<vec4<f32>, 10>,",
                "paint: vec4<f32>,",
                "lamps: array<vec4<f32>, 8>,",
                "wet: array<vec4<f32>, 3>,",
            ]
        );
        // 4 + 1 + 6 + 1 + 1 + 10 + 1 + 8 + 3 vec4s = 560 bytes.
        assert_eq!((4 + 1 + 6 + 1 + 1 + 10 + 1 + 8 + 3) * 16, DRAW_STRIDE);
    }

    /// The native module is car.wgsl alone (so Enhanced Car Paint off leaves
    /// the native shader untouched); the paint module routes only the final
    /// combine through `paint_layer`.
    #[test]
    fn paint_module_only_replaces_the_final_combine() {
        let native = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car.wgsl"));
        assert!(!native.contains("paint_layer") && !native.contains("enhanced_paint("));
        let paint = paint_source().unwrap();
        assert_eq!(paint.matches("r0 = paint_layer(").count(), 1);
        assert!(!paint.contains("    r0 = 2.0 * r0 * r1.w + r1.xyz;\n"));
        let (head, tail) = paint.split_at(
            native.len() + "paint_layer(in, r0, env, texel.rgb, specular, r1, direction)".len()
                - "2.0 * r0 * r1.w + r1.xyz".len(),
        );
        assert_eq!(
            head.replace("paint_layer(in, r0, env, texel.rgb, specular, r1, direction)", "2.0 * r0 * r1.w + r1.xyz"),
            native
        );
        assert_eq!(tail, include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car_paint.wgsl")));
        assert_eq!(<CarMode as CarModeExt>::NATIVE, (crate::render::SceneMode::NATIVE, false, false, false));
        assert_eq!(module_index(<CarMode as CarModeExt>::NATIVE), 0);
    }

    /// Spots like raindrops on the paint (docs/notes/graphics/car-look.md "Spots like raindrops on a dry car"):
    /// the flake must read as paint, not water. Each cell's glint is the
    /// filtered mean of the sub-pixel flakes' lobe with a bounded spread, not
    /// a single tilted normal's glint (isolated bright cells), and the grain
    /// stays faint enough that Sharpen 100 cannot turn it into dots. The
    /// filtered lobe keeps the old flake's mean (Monte Carlo of the old
    /// model: 40 % of the cells, normal tilted +-0.35 per axis, power 20).
    #[test]
    fn flake_is_filtered_not_droplets() {
        let wgsl = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car_paint.wgsl"));
        let konst = |name: &str| -> f32 {
            let line = wgsl
                .lines()
                .find(|l| l.starts_with(&format!("const {name}: f32 = ")))
                .unwrap_or_else(|| panic!("{name}"));
            let expr = line.split('=').nth(1).unwrap().trim().trim_end_matches(';');
            expr.split('*').map(|f| f.trim().parse::<f32>().unwrap()).product()
        };
        assert!(!wgsl.contains("rnd.x > 0.6"), "binary per-cell flake glint is back");
        assert!(!wgsl.contains("normalize(n + (rnd"), "per-cell tilted-normal glint is back");
        let (mean, power, spread, grain) =
            (konst("FLAKE_MEAN"), konst("FLAKE_POWER"), konst("FLAKE_SPREAD"), konst("FLAKE_GRAIN"));
        assert!(spread <= 1.0, "a cell may vary at most +-50 % around the mean");
        assert!(grain <= 0.02, "grain {grain}");
        // Old model's mean, deterministic LCG, light offset 0..40 degrees.
        let mut seed = 0x1234_5678u64;
        let mut rnd = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((seed >> 33) as f64 / (1u64 << 31) as f64) as f32
        };
        for deg in [0.0f32, 10.0, 20.0, 30.0, 40.0] {
            let v = glam::Vec3::new(0.6428, 0.0, 0.766);
            let n = glam::Vec3::Z;
            let r = 2.0 * v.dot(n) * n - v;
            let l = glam::Quat::from_rotation_y(deg.to_radians()) * r;
            let samples = 20000;
            let mut sum = 0.0;
            for _ in 0..samples {
                let q = glam::Vec3::new(rnd() * 2.0 - 1.0, rnd() * 2.0 - 1.0, rnd() * 2.0 - 1.0);
                let lit = rnd() > 0.6;
                let nf = (n + q * 0.35).normalize();
                let rf = 2.0 * v.dot(nf) * nf - v;
                if lit {
                    sum += rf.dot(l).max(0.0).powf(20.0);
                }
            }
            let old = sum / samples as f32;
            let new = mean * r.dot(l).max(0.0).powf(power);
            assert!((new - old).abs() < 0.02, "{deg} deg: filtered {new} vs old mean {old}");
        }
    }

    #[test]
    fn paint_types_by_light_material() {
        for (name, t) in [
            ("REGPAINTRED", PaintType::Gloss),
            ("METPAINTSILVER", PaintType::Metallic),
            ("METPAINT_12", PaintType::Metallic),
            ("PEARL", PaintType::Pearl),
            ("PEARL7", PaintType::Pearl),
            ("RIMPEARL3", PaintType::Pearl),
            ("CUSTOMPAINT_14", PaintType::Candy),
            ("CHROME", PaintType::Chrome),
            ("MAGCHROME", PaintType::Chrome),
            ("USER_RIMS_CHROME", PaintType::Chrome),
            ("MAGSILVER", PaintType::Metallic),
            ("USER_RIMS_GLOSS", PaintType::Gloss),
            ("WINDSHIELD", PaintType::Native),
            ("RUBBER", PaintType::Native),
            ("DULLPLASTIC", PaintType::Native),
            ("MATTEPLASTIC", PaintType::Native),
            ("HEADLIGHTGLASS", PaintType::Native),
            ("DECAL", PaintType::Native),
        ] {
            assert_eq!(PaintType::of(name), t, "{name}");
        }
        // Native materials carry no layer (car.wgsl skips it on all-zero).
        assert_eq!(PaintType::Native.params(), [0.0; 4]);
        for t in [PaintType::Gloss, PaintType::Metallic, PaintType::Pearl, PaintType::Candy, PaintType::Chrome] {
            assert!(t.params().iter().any(|&v| v != 0.0));
        }
    }

    /// Every body paint part of the paint shop (slot 0x3F) gets the type of
    /// its finish (brand GLOSS / METAL / PEARL), and every light material
    /// named like a paint is classified.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn every_paint_part_has_its_finish_type() {
        let root = crate::testkit::game_root();
        let data = std::fs::read(root.join("GLOBAL/GLOBALB.BUN")).unwrap();
        let db = crate::formats::parts::PartDb::load(&data).unwrap();
        let materials = crate::car::parse_light_materials(&data);
        let mut seen = std::collections::HashMap::<PaintType, usize>::new();
        let mut after = None;
        while let Some(p) = db.find_after("350Z", 0x3F, after) {
            after = Some(p);
            let brand = db.attribute(p, 0xEBB0_3E66).unwrap_or(0);
            let Some(m) = db.attribute(p, 0x6BA0_2C05).and_then(|h| materials.get(&h)) else { continue };
            let t = PaintType::of(&m.name);
            let want: &[PaintType] = if brand == bin_hash("GLOSS") {
                &[PaintType::Gloss]
            } else if brand == bin_hash("METAL") {
                &[PaintType::Metallic]
            } else if brand == bin_hash("PEARL") {
                &[PaintType::Pearl, PaintType::Candy]
            } else {
                continue;
            };
            assert!(want.contains(&t), "{} → {t:?}", m.name);
            *seen.entry(t).or_default() += 1;
        }
        for t in [PaintType::Gloss, PaintType::Metallic, PaintType::Pearl, PaintType::Candy] {
            assert!(seen.get(&t).copied().unwrap_or(0) >= 5, "{t:?}: {seen:?}");
        }
        for m in materials.values() {
            if ["REGPAINT", "METPAINT", "PEARL", "RIMPEARL", "CUSTOMPAINT"].iter().any(|p| m.name.starts_with(p)) {
                assert_ne!(PaintType::of(&m.name), PaintType::Native, "{}", m.name);
            }
        }
    }
}

#[cfg(test)]
mod ssr_mask_tests {
    use super::*;

    /// The SSR mask module is car.wgsl with car_ssr.wgsl's fragment stages
    /// appended; the cars' own module (native and paint) has none of it.
    #[test]
    fn mask_module_appends_to_the_native_car_shader() {
        let src = ssr_mask_source(false).unwrap();
        assert_eq!(
            src,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car.wgsl")).to_string()
                + include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car_ssr.wgsl"))
        );
        assert!(src.contains("fn fs_ssr_mask(") && src.contains("fn fs_ssr_mask_alpha_test("));
        assert!(!include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car.wgsl")).contains("ssr_mask"));
        assert!(!paint_source().unwrap().contains("ssr_mask"));
        // Its pipelines' override and the mask's three outputs.
        assert!(src.contains("override MASK_BLENDED"));
        assert!(
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car_ssr.wgsl")).matches("@location(").count()
                >= 3
        );
    }

    /// Surface Detail on cars (G7c2): the native module and the modules
    /// without the row are untouched; with it, car_colour takes the relief
    /// once (env turn and specular before the cube's sample, the diffuse),
    /// on top of Enhanced Paint and Wet Car; the SSR mask's normal takes the
    /// map with its derivatives at the top.
    #[test]
    fn surface_module_hooks_car_colour_only_when_on() {
        let native = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car.wgsl"));
        assert_eq!(car_source_n(false, false, false).unwrap(), native);
        for (paint, wet) in [(false, false), (true, false), (false, true), (true, true)] {
            assert_eq!(car_source_n(paint, wet, false).unwrap(), car_source(paint, wet).unwrap());
            assert!(!car_source(paint, wet).unwrap().contains("surface_relief"));
            let s = car_source_n(paint, wet, true).unwrap();
            assert_eq!(s.matches("let relief = surface_relief(in);").count(), 1);
            assert_eq!(s.matches("in.diffuse.xyz * relief.diffuse").count(), 1);
            assert!(s.ends_with(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car_surface.wgsl"))));
            // The relief comes before the cube's sample, which stays the only one.
            assert!(s.find("surface_relief(in)").unwrap() < s.find(SURFACE_ENV).unwrap());
            assert_eq!(s.matches(SURFACE_ENV).count(), 1);
        }
        let mask = ssr_mask_source(true).unwrap();
        assert_eq!(mask.matches("let surface_nw = surface_world_normal(in, nw0);").count(), 1);
        let body = &mask[mask.find("fn ssr_mask(").unwrap()..];
        assert!(body.find("let surface_nw").unwrap() < body.find("discard").unwrap());
        assert!(mask.contains("var nw = surface_nw;"));
        // The modules by mode never collide.
        let modes: Vec<usize> = (0..16)
            .map(|i| {
                module_index((
                    crate::render::SceneMode { bias_q: (i >> 3) as i8, ..crate::render::SceneMode::NATIVE },
                    i & 1 != 0,
                    i & 2 != 0,
                    i & 4 != 0,
                ))
            })
            .collect();
        assert_eq!(modes, (0..16).collect::<Vec<_>>());
    }
    /// No car module indexes the private per-draw copy with a
    /// loop variable (that kept the whole record in scratch memory on every
    /// pixel); the lamps are read from the storage buffer.
    #[test]
    fn car_modules_index_no_private_record() {
        for (paint, wet, surface) in [
            (false, false, false),
            (true, false, false),
            (false, true, false),
            (true, true, false),
            (false, false, true),
            (true, true, true),
        ] {
            let src = car_source_n(paint, wet, surface).unwrap();
            assert!(!src.contains("draw.lamps[i"), "{paint} {wet} {surface}");
            assert_eq!(src.matches("draws[in.draw_index].lamps[i * 2u").count(), 2);
        }
        let glass = super::glass::glass_source(true, true).unwrap();
        assert!(!glass.contains("draw.lamps[i"));
    }
}
