//! Car lighting as the game builds it for each car every frame (0x620AF0
//! → 0x48ED60, 0x48F660; uploaded by 0x5BD150 as car.efx's HarmonicCoeff
//! and LocalEyePos).
//!
//! The light sources are the scene's three directional lights (in game the
//! static setup at 0x7F3F00, all three fixed to the camera) and up to ten
//! of the track's lights near the car (0x48E3D0, from the light packs).
//! Each light becomes a second-order spherical-harmonic lobe (0x4895E0),
//! written as the quadratic form car.wgsl evaluates; the sum's specular
//! channel is rescaled (0x489ED0), then the form is turned into the car's
//! own axes (0x489BA0).

use glam::{Mat3, Mat4, Vec3, Vec4};

use crate::formats::lights::{LightPack, WorldLight};

// The record type sits with the engine's tables (`exe_tables::records`).
pub use crate::exe_tables::records::RigLight;

/// The in-game setup (0x7F3F00, static data nothing writes): a key light
/// from straight above the camera, a fill and a rim, all white.
pub static IN_GAME_RIG: crate::exe_tables::Table<[RigLight; 3]> = crate::exe_tables::Table::new(|t| &t.in_game_rig);

/// The default setup (0x7F3E10, static data): what 0x48A2D0 returns for a
/// name no loaded setup has; render infos start with it at +0x904
/// (0x63B89B).
pub static DEFAULT_RIG: crate::exe_tables::Table<[RigLight; 3]> = crate::exe_tables::Table::new(|t| &t.default_rig);

/// Chunk of named light setups (FRONTEND/FrontB; 0x48A280 keeps its
/// records for 0x48A2D0's lookup by name hash).
pub const LIGHT_SETUPS: u32 = 0x0003_B700;

/// The named light setups in a bundle: records of 0x74 bytes, the name
/// hash at +0 and the three lights at +0x20 + i·0x1C. The front end's
/// platform scenes (0x4A7180) look theirs up by the scene's name (SHOWROOM,
/// CRIB01, …; table 0x7F60E8) into the car's render info +0x904, which
/// 0x63121C hands to 0x620AF0; in game 0x6316A6 passes 0x7F3F00 instead.
pub fn parse_light_setups(bundle: &[u8]) -> std::collections::HashMap<u32, [RigLight; 3]> {
    let mut setups = std::collections::HashMap::new();
    crate::formats::bundle::walk(crate::formats::bundle::chunks(bundle, 0), &mut |c| {
        if c.id != LIGHT_SETUPS {
            return;
        }
        for r in c.body.chunks_exact(0x74) {
            let f = |at: usize| f32::from_le_bytes(r[at..at + 4].try_into().unwrap());
            let light = |i: usize| {
                let at = 0x20 + 0x1C * i;
                RigLight {
                    camera_relative: u32::from_le_bytes(r[at..at + 4].try_into().unwrap()) != 0,
                    theta: f(at + 4),
                    phi: f(at + 8),
                    colour: Vec3::new(f(at + 12), f(at + 16), f(at + 20)),
                    intensity: f(at + 24),
                }
            };
            setups.insert(u32::from_le_bytes(r[0..4].try_into().unwrap()), [light(0), light(1), light(2)]);
        }
    });
    setups
}

/// The axes of the game's camera as its look-at (0x5BA9B0) builds them:
/// forward, right = (−up) × forward, and "down" = forward × right.
#[derive(Clone, Copy, Debug)]
pub struct CameraAxes {
    pub right: Vec3,
    pub down: Vec3,
    pub forward: Vec3,
}

impl CameraAxes {
    pub fn look(forward: Vec3, up: Vec3) -> CameraAxes {
        let forward = forward.normalize_or(Vec3::Y);
        let right = (-up).cross(forward).normalize_or(Vec3::X);
        let down = forward.cross(right).normalize_or(-Vec3::Z);
        CameraAxes { right, down, forward }
    }
}

/// The game's binary angle for `deg` degrees (`(int)(deg · 65536) / 360`,
/// kept to 16 bits) as radians.
fn binary_angle(deg: f32) -> f32 {
    let a = ((deg * 65536.0) as i32 / 360) as u16;
    a as f32 * (std::f32::consts::TAU / 65536.0)
}

/// 0x43C350: the sine of an angle in radians, through the game's binary
/// angle (`(int)(x · 65536 / 2π)`, 16 bits).
fn sin_radians(x: f32) -> f32 {
    let a = (x * (65536.0 / std::f32::consts::TAU)) as i32 as u16;
    (a as f32 * (std::f32::consts::TAU / 65536.0)).sin()
}

/// A rig light's direction (0x48A310 with θ + 180° and 90° − φ, then, for
/// a camera light, 0x48ED60's (−y, −z, x) in camera axes).
pub fn rig_direction(light: &RigLight, camera: &CameraAxes) -> Vec3 {
    let (sa, ca) = binary_angle(light.theta + 180.0).sin_cos();
    let (sb, cb) = binary_angle(90.0 - light.phi).sin_cos();
    let d = Vec3::new(sb * ca, sb * sa, cb);
    if light.camera_relative { -d.y * camera.right - d.z * camera.down + d.x * camera.forward } else { d }
}

/// A light as the harmonic sees it: unit direction towards it, colour.
struct Lobe {
    direction: Vec3,
    colour: Vec3,
}

/// The light packs' lights near `at` (0x48E3D0 → 0x4893A0): per pack, the
/// lights listed by the tree leaf holding `at`; a light counts while on
/// and within its outer radius, weighted 1 inside the inner radius and
/// fading to 0 at the outer one. Only point (2) and spot (3) lights light
/// the car; then the scene light groups (list 0x835850). The pack walk
/// stops once more than nine lights are stored; the scene groups have no
/// such check and stop only at 0x4893A0's 24 stored lights.
fn nearby(packs: &[&LightPack], scene: &[WorldLight], at: Vec3) -> Vec<Lobe> {
    let mut found = Vec::new();
    'packs: for pack in packs {
        let Some(lights) = pack.leaf_lights(at) else { continue };
        for light in lights {
            if found.len() > 9 {
                break 'packs;
            }
            found.extend(consider(light, at));
        }
    }
    // Then the light groups of the list 0x835850 (0x48E550…0x48E5CB): each
    // group whose +0x28 is set, its lights through the same 0x4893A0
    // (**verified**: 0x48E3D0's `9 < count` test guards the
    // pack loop only; 0x4893A0 stores at most 0x18 lights, `cmp …,0x18`).
    for light in scene {
        if found.len() >= MAX_STORED_LIGHTS {
            break;
        }
        found.extend(consider(light, at));
    }
    found
}

/// The lights 0x4893A0 stores at most (0x18, its `cmp …,0x18`).
const MAX_STORED_LIGHTS: usize = 24;

/// 0x4893A0 for one light: on (+7), inside its outer radius (+0x1C),
/// weight 1 inside the inner (+0x30) else falling to 0 by d²; point (2)
/// and spot (3) lights only.
fn consider(light: &WorldLight, at: Vec3) -> Option<Lobe> {
    if !light.on {
        return None;
    }
    let d2 = (at - light.position).length_squared();
    let outer2 = light.outer * light.outer;
    if d2 >= outer2 {
        return None;
    }
    let inner2 = light.inner * light.inner;
    let weight = if inner2 < d2 { 1.0 - (d2 - inner2) / (outer2 - inner2) } else { 1.0 };
    if !matches!(light.kind, 2 | 3) {
        return None;
    }
    // 0x48ED60: spots fade over their cone, cos(cone / 2) to 1.
    let mut w = weight;
    if light.kind == 3 {
        let threshold = sin_radians(light.cone * 0.5 + std::f32::consts::FRAC_PI_2);
        let f = (at - light.position).normalize_or_zero().dot(light.direction);
        w *= if f <= threshold { 0.0 } else { (f - threshold) / (1.0 - threshold) };
    }
    let c = Vec3::new(light.colour[0] as f32, light.colour[1] as f32, light.colour[2] as f32);
    Some(Lobe { direction: light.position - at, colour: c * 0.75 * w / 255.0 })
}

/// Add one light's second-order spherical-harmonic lobe (0x4895E0), as the
/// quadratic c0 + c1x² + c2y² + c3z² + c4xy + c5xz + c6yz + c7x + c8y + c9z,
/// to all four channels.
fn add_lobe(h: &mut [Vec4; 10], colour: Vec4, d: Vec3) {
    let (x, y, z) = (d.x, d.y, d.z);
    let e = z * z * 3.0 - 1.0;
    let g = x * x - y * y;
    let c = colour;
    let band2 = c * 0.315392 * e;
    h[0] += c * 0.282095 * 0.886227 - band2 * 0.247708;
    let xy_diff = c * 0.546274 * g * 0.429043;
    h[1] += xy_diff;
    h[2] -= xy_diff;
    h[3] += band2 * 0.743125;
    let xc = c * x * 1.092548;
    h[4] += xc * y * 0.858086;
    h[5] += xc * z * 0.858086;
    h[6] += c * 1.092548 * y * z * 0.429043;
    let band1 = c * 0.488603 * 1.023328;
    h[7] += band1 * x;
    h[8] += band1 * y;
    h[9] += band1 * z;
}

/// (Clone deviation, docs/notes/graphics/car-look.md "Higher settings"): with
/// the scene specular the car's environment reflection (car.efx's
/// `2 · env · (EnvmapMin + fresnel · EnvmapRange)`, the world cube) is
/// scaled by this, so the scene the paint reflects carries the body where
/// no lamp is near, in place of the rig's camera-fixed highlight. It adds
/// no lobe of its own, so nothing lands on the panel that faces the
/// camera. **Inferred** (tuned): the largest gain that keeps the 350Z's
/// trunk in the intro at the level of its roof and quarters (no local
/// peak) and no car brighter than the Original.
pub const SCENE_ENV_GAIN: f32 = 1.15;

/// The quadratic form evaluated at a direction `n` taken in axes whose
/// columns are `r` (0x489BA0): q'(n) = q(r · n).
fn rotate(h: &[Vec4; 10], r: Mat3) -> [Vec4; 10] {
    let (a, b, c) = (r.x_axis, r.y_axis, r.z_axis);
    let quad = |p: Vec3, q: Vec3| {
        // pᵀ A q with A the symmetric matrix of the x², …, yz terms.
        h[1] * (p.x * q.x)
            + h[2] * (p.y * q.y)
            + h[3] * (p.z * q.z)
            + h[4] * ((p.x * q.y + p.y * q.x) * 0.5)
            + h[5] * ((p.x * q.z + p.z * q.x) * 0.5)
            + h[6] * ((p.y * q.z + p.z * q.y) * 0.5)
    };
    let lin = |p: Vec3| h[7] * p.x + h[8] * p.y + h[9] * p.z;
    [
        h[0],
        quad(a, a),
        quad(b, b),
        quad(c, c),
        quad(a, b) * 2.0,
        quad(a, c) * 2.0,
        quad(b, c) * 2.0,
        lin(a),
        lin(b),
        lin(c),
    ]
}

/// HarmonicCoeff and LocalEyePos for one car.
#[derive(Clone, Copy, Debug)]
pub struct CarLighting {
    /// Ten terms, rgb the colour and w the specular intensity.
    pub harmonic: [Vec4; 10],
    /// The eye in the car's axes at a fixed distance (0x48A6A0).
    pub local_eye: Vec3,
    /// The gathered lights before they are turned into the car's axes, the
    /// eye (world) and the mode, kept so a part with its own light context
    /// can be lit in its own axes ([`CarLighting::in_axes`]).
    pub(super) world_harmonic: [Vec4; 10],
    pub(super) eye: Vec3,
    pub(super) frontend: bool,
    /// Dynamic Lamps (clone deviation, docs/notes/graphics/lamps-shadows.md): the four
    /// light-pack lamps that light the car most, per pixel in car.wgsl
    /// (`Draw.lamps`); zeros natively.
    pub lamps: [[f32; 4]; 8],
    /// The factor on the environment reflection's EnvmapMin / EnvmapRange
    /// (1 natively; [`SCENE_ENV_GAIN`] with the scene specular).
    pub env_gain: f32,
}

impl CarLighting {
    /// Light a car at `car_world` (car → world) seen from `eye` along
    /// `camera`. `frontend` is the garage scenes' mode (0x8654A4 == 3).
    pub fn compute(
        rig: &[RigLight; 3],
        packs: &[&LightPack],
        scene: &[WorldLight],
        camera: &CameraAxes,
        eye: Vec3,
        car_world: Mat4,
        frontend: bool,
    ) -> CarLighting {
        CarLighting::compute_with(rig, packs, scene, camera, eye, car_world, frontend, false)
    }

    /// [`CarLighting::compute`]; with `scene_specular` (clone
    /// deviation, docs/notes/graphics/car-look.md "Car highlight from the scene, not the camera rig") the specular channel (w) holds
    /// the scene's lights only: the same lobes and the same 0x489ED0 scale
    /// as natively (0.8 over the length of every light's w, rig included),
    /// less the rig's lobes. The colour channels (diffuse, the highlight's
    /// tint) stay native, so the rig still fills the body; only its
    /// camera-fixed highlight goes, with no stand-in lobe: the environment
    /// reflection carries the body instead ([`CarLighting::env_gain`]).
    #[allow(clippy::too_many_arguments)]
    pub fn compute_with(
        rig: &[RigLight; 3],
        packs: &[&LightPack],
        scene: &[WorldLight],
        camera: &CameraAxes,
        eye: Vec3,
        car_world: Mat4,
        frontend: bool,
        scene_specular: bool,
    ) -> CarLighting {
        let at = car_world.w_axis.truncate();
        let mut lobes: Vec<Lobe> =
            rig.iter().map(|l| Lobe { direction: rig_direction(l, camera), colour: l.colour * l.intensity }).collect();
        lobes.extend(nearby(packs, scene, at));

        // 0x48ED60: unit directions, the specular channel the colour's
        // luminance, each light plus a −0.05 lobe opposite (0x489AF0).
        let mut h = [Vec4::ZERO; 10];
        // The scene lights' specular channel alone (the lobes after
        // the rig's three).
        let mut scene_w = [0.0f32; 10];
        for (i, l) in lobes.iter().enumerate() {
            let d = l.direction.normalize_or_zero();
            let c = l.colour;
            let colour = Vec4::new(c.x, c.y, c.z, c.y * 0.54 + c.z * 0.17 + c.x * 0.29);
            add_lobe(&mut h, colour, d);
            add_lobe(&mut h, colour * -0.05, -d);
            if scene_specular && i >= rig.len() {
                let mut one = [Vec4::ZERO; 10];
                add_lobe(&mut one, colour, d);
                add_lobe(&mut one, colour * -0.05, -d);
                for (w, v) in scene_w.iter_mut().zip(one) {
                    *w += v.w;
                }
            }
        }
        // 0x489ED0: the colour channels keep their size (0x82DAE4 is 0);
        // the specular channel is scaled to a length of 0.8 (0x7F39C0).
        let length = h.iter().map(|v| v.w * v.w).sum::<f32>().sqrt();
        if length > 0.0 {
            let scale = 0.8 / length;
            for (v, w) in h.iter_mut().zip(scene_w) {
                v.w = if scene_specular { w * scale } else { v.w * scale };
            }
        }
        let (harmonic, local_eye) = axes_lighting(&h, eye, car_world, frontend);
        let env_gain = if scene_specular { SCENE_ENV_GAIN } else { 1.0 };
        CarLighting { harmonic, local_eye, world_harmonic: h, eye, frontend, lamps: [[0.0; 4]; 8], env_gain }
    }

    /// Whether this is the front end's lighting (0x8654A4 == 3).
    pub fn is_frontend(&self) -> bool {
        self.frontend
    }

    /// The same lights in another model's axes: 0x48F660 run again with
    /// that model's world matrix. The wheel draw does so per wheel and
    /// brake (0x620AF0: 0x6286E7…, each wheel's matrix including its 180°
    /// turn, into its own 0x1C0-byte context), so a turned left wheel is lit
    /// in its own axes rather than the car's.
    pub fn in_axes(&self, world: Mat4) -> ([Vec4; 10], Vec3) {
        axes_lighting(&self.world_harmonic, self.eye, world, self.frontend)
    }
}

/// 0x48F660: the harmonic turned into the axes of `world` (0x489BA0) and
/// the eye in those axes, its xyz length clamped to 2.78, or
/// 2.78–8 in the frontend (0x48A6A0).
pub(super) fn axes_lighting(h: &[Vec4; 10], eye: Vec3, world: Mat4, frontend: bool) -> ([Vec4; 10], Vec3) {
    let rotation = Mat3::from_mat4(world);
    let harmonic = rotate(h, rotation);
    let local = rotation.transpose() * (eye - world.w_axis.truncate());
    // 0x5BB1A0 clears the transpose's homogeneous row/column, so the
    // local eye has w=0 when 0x48A7E8 measures its length.
    let length = local.length();
    let distance = length.clamp(2.78, if frontend { 8.0 } else { 2.78 });
    (harmonic, local.normalize_or_zero() * distance)
}

// Refactor: the old path `render::lighting::WorldLights`.
pub use super::lighting_world::WorldLights;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_eye_uses_xyz_length_and_preserves_frontend_interior_distance() {
        for length in [0.0f32, 1.0, 2.77, 2.78, 3.0, 7.99, 8.0, 9.0] {
            for frontend in [false, true] {
                let (_, eye) = axes_lighting(&[Vec4::ZERO; 10], Vec3::X * length, Mat4::IDENTITY, frontend);
                let expected = if length == 0.0 { 0.0 } else { length.clamp(2.78, if frontend { 8.0 } else { 2.78 }) };
                assert!(eye.abs_diff_eq(Vec3::X * expected, 0.000001), "length{length} frontend{frontend}: {eye:?}");
            }
        }
    }

    #[test]
    fn scene_lights_follow_the_packs_within_their_radius() {
        // The flag girl's flare light (0x433090): point, r g b 0xDF 0x91
        // 0xA9, outer 3.7, inner 0; 0x4893A0 weighs it 1 − d²/3.7².
        let flare = WorldLight {
            kind: 2,
            sub: 2,
            on: true,
            colour: [0xDF, 0x91, 0xA9],
            position: Vec3::new(2.0, 0.0, 0.0),
            outer: 3.7,
            direction: Vec3::ZERO,
            intensity: 0.0,
            inner: 0.0,
            cone: 14000.0,
        };
        let found = nearby(&[], std::slice::from_ref(&flare), Vec3::ZERO);
        assert_eq!(found.len(), 1);
        let w = 1.0 - 4.0 / (3.7 * 3.7);
        assert!((found[0].colour.x - 223.0 * 0.75 * w / 255.0).abs() < 1e-5);
        assert_eq!(found[0].direction, Vec3::new(2.0, 0.0, 0.0));
        assert!(nearby(&[], &[flare], Vec3::new(-2.0, 0.0, 0.0)).is_empty());
    }

    /// 0x48E3D0's ten-light test guards only the pack walk;
    /// the scene groups (0x835850) go on to 0x4893A0's 24 stored lights.
    #[test]
    fn scene_groups_are_not_capped_at_ten() {
        let light = |x: f32| WorldLight {
            kind: 2,
            sub: 2,
            on: true,
            colour: [0xFF, 0xFF, 0xFF],
            position: Vec3::new(x, 1.0, 0.0),
            outer: 50.0,
            direction: Vec3::ZERO,
            intensity: 0.0,
            inner: 0.0,
            cone: 14000.0,
        };
        let scene: Vec<WorldLight> = (0..30).map(|i| light(i as f32 * 0.1)).collect();
        assert_eq!(nearby(&[], &scene[..15], Vec3::ZERO).len(), 15);
        assert_eq!(nearby(&[], &scene, Vec3::ZERO).len(), 24);
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn the_key_light_comes_from_above_the_camera() {
        let camera = CameraAxes::look(Vec3::Y, Vec3::Z);
        assert!((camera.down - -Vec3::Z).length() < 1e-6);
        let d = rig_direction(&IN_GAME_RIG[0], &camera);
        assert!((d - Vec3::Z).length() < 1e-3, "{d}");
    }

    /// With the scene specular, the rig's camera-fixed
    /// highlight is gone (w holds the scene lights only, at the native
    /// scale, with no stand-in), the colour channels are the native ones,
    /// and without it the context is native.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn scene_specular_drops_the_rigs_highlight_only() {
        let camera = CameraAxes::look(Vec3::Y, Vec3::Z);
        let world = Mat4::from_translation(Vec3::new(10.0, 20.0, 0.0));
        let eye = Vec3::new(10.0, 15.0, 2.0);
        let lamp = WorldLight {
            kind: 2,
            sub: 2,
            on: true,
            colour: [110, 110, 125],
            position: Vec3::new(14.0, 20.0, 6.0),
            outer: 30.0,
            direction: Vec3::ZERO,
            intensity: 0.0,
            inner: 10.0,
            cone: 0.0,
        };
        let native = CarLighting::compute(&IN_GAME_RIG, &[], std::slice::from_ref(&lamp), &camera, eye, world, false);
        let off = CarLighting::compute_with(
            &IN_GAME_RIG,
            &[],
            std::slice::from_ref(&lamp),
            &camera,
            eye,
            world,
            false,
            false,
        );
        let scene =
            CarLighting::compute_with(&IN_GAME_RIG, &[], std::slice::from_ref(&lamp), &camera, eye, world, false, true);
        assert_eq!(native.harmonic, off.harmonic);
        for (a, b) in native.harmonic.iter().zip(scene.harmonic) {
            assert_eq!(a.truncate(), b.truncate());
        }
        // w: the lamp's lobe (and its −0.05 twin) alone: the lamp (+x of
        // the car) is the only sideways pull, and nothing stands in for the
        // rig (docs/notes/graphics/car-look.md "Higher settings").
        let lin = Vec3::new(scene.world_harmonic[7].w, scene.world_harmonic[8].w, scene.world_harmonic[9].w);
        assert!(lin.x > 0.05 && lin.y.abs() < 1e-4, "{lin}");
        // The rig alone: no highlight at all (w zero in every direction),
        // the diffuse fill kept, the environment reflection carrying the body.
        let rig = CarLighting::compute_with(&IN_GAME_RIG, &[], &[], &camera, eye, world, false, true);
        assert!(rig.world_harmonic.iter().all(|v| v.w == 0.0), "{:?}", rig.world_harmonic);
        assert!(rig.harmonic[0].x > 0.1);
        assert_eq!(rig.env_gain, SCENE_ENV_GAIN);
        assert_eq!(native.env_gain, 1.0);
        assert_eq!(off.env_gain, 1.0);
    }

    #[test]
    fn rotating_the_form_matches_evaluating_it_rotated() {
        let mut h = [Vec4::ZERO; 10];
        add_lobe(&mut h, Vec4::new(1.0, 0.5, 0.25, 1.0), Vec3::new(0.3, -0.5, 0.8).normalize());
        let eval = |h: &[Vec4; 10], d: Vec3| {
            h[0] + h[1] * d.x * d.x
                + h[2] * d.y * d.y
                + h[3] * d.z * d.z
                + h[4] * d.x * d.y
                + h[5] * d.x * d.z
                + h[6] * d.z * d.y
                + h[7] * d.x
                + h[8] * d.y
                + h[9] * d.z
        };
        let r = Mat3::from_rotation_z(0.7) * Mat3::from_rotation_x(-0.3);
        let local = rotate(&h, r);
        let n = Vec3::new(-0.2, 0.9, 0.1).normalize();
        assert!((eval(&local, n) - eval(&h, r * n)).length() < 1e-5);
    }
}
