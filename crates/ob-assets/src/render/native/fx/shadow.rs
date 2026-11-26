//! The car shadow (the car draw 0x6162F0, 0x616D40..0x617411): a 4×4 grid
//! of vertices over the car's box, laid on the ground, drawn as three
//! strips with the car's `%s_SHADOWIG` texture (`%s_SHADOWFE` in the front
//! end). See docs/formats.md, "Car shadow".

use glam::{Mat4, Vec2, Vec3};

#[cfg(test)]
use super::neon::*;

/// The grid's corner wheel per vertex (table built at 0x616E2F..0x616ECD):
/// the wheel whose ground plane stands in for a vertex the query misses.
/// Wheels in the car type's order (front left, front right, rear right,
/// rear left); rows run from the box's min y (right) and columns from its
/// min x (rear).
const VERTEX_WHEEL: [usize; 16] = [2, 2, 1, 1, 2, 2, 1, 1, 3, 3, 0, 0, 3, 3, 0, 0];

/// How far Car_Draw pulls the shadow toward the eye (0x7A66CC).
pub const SHADOW_PULL: f32 = 0.104;

/// The pull while the first player's camera mover is not the chase camera
/// (0x7A66D0; 0x62136E..0x62138F): player 0 (0x8900B4, the player list
/// filled by the player constructor 0x5F1680) has a mover at +0x238 whose
/// type +0x0C (the type Camera_Factory 0x5F1FE0 built) is not 1 — a track
/// camera (0x12), the showcase (0x14), an NIS camera … In the front end
/// there is no player and the pull is `SHADOW_PULL`.
pub const SHADOW_PULL_OTHER_CAMERA: f32 = 0.023;

/// The pull for a frame: `mover` the first player's camera mover type, or
/// None without a player (0x62133C..0x62138F).
pub fn shadow_pull(mover: Option<u32>) -> f32 {
    match mover {
        Some(t) if t != 1 => SHADOW_PULL_OTHER_CAMERA,
        _ => SHADOW_PULL,
    }
}

/// One grid vertex: world position and texture coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadowVertex {
    pub position: Vec3,
    pub uv: [f32; 2],
}

/// A car's shadow for one frame.
#[derive(Clone, Debug)]
pub struct Shadow {
    /// Row-major (row = y step, column = x step).
    pub vertices: [ShadowVertex; 16],
    /// Strip `j` joins rows `j` and `j + 1`; false when skipped.
    pub strips: [bool; 3],
    /// The vertex colour's alpha (colour 0x808080 | alpha << 24).
    pub alpha: u8,
    /// The vertex colour as the vertex gets it (0x5BE0B0: D3DCOLOR,
    /// bytes B, G, R, A in memory): (0x80, 0x80, 0x80, `alpha`) for the
    /// shadow, the neon's colour for [`Shadow::neon`].
    pub color: [u8; 4],
    /// clamp(up.z, 0, 1)·(1 − t) (0x6226DE..0x622725, `[esp+0xC0]`): the
    /// underglow's alpha factor.
    pub fade: f32,
    /// The Graphics page's Shadow Quality (clone deviation): the kept
    /// strips' quads split finer and laid on the ground ([`Shadow::refine`]);
    /// None: the native grid.
    pub refined: Option<Vec<[ShadowVertex; 4]>>,
}

impl Shadow {
    /// The grid moved toward the eye, as Car_Draw (0x620AF0) draws it:
    /// 0x48AE30 scales the shadow's matrix (identity in the front end, the
    /// grid being in world space) about the view's eye (view +0x40 →
    /// camera +0x40) by `s = (d − pull) / d` (0x62133C..0x621365, 1.0
    /// 0x784250), `d` the distance from the eye to the car's position
    /// (0x43CE10 at 0x621277), `pull` 0.104 m (0x7A66CC), or 0.023 m
    /// with a non-chase camera (`shadow_pull`). It is a depth bias: on screen nothing
    /// moves, but the grid at 0.152 m wins over SHOWROOM's disc tops
    /// (0.159 m). Checked against the apitrace of the Quick Race car select
    /// (t1.trace call 10645088: world.efx's WorldViewProj for the shadow is
    /// the view's scaled by 0.98903 = (9.48 − 0.104) / 9.48, the eye 9.48 m
    /// from the car on disc 0).
    pub fn toward_eye(mut self, eye: Vec3, car_position: Vec3, pull: f32) -> Shadow {
        let d = (car_position - eye).length();
        if d > 0.0 {
            let s = (d - pull) / d;
            for v in &mut self.vertices {
                v.position = eye + (v.position - eye) * s;
            }
            for v in self.refined.iter_mut().flatten().flatten() {
                v.position = eye + (v.position - eye) * s;
            }
        }
        self
    }

    /// The strips as quads, corners in the order (b0, a0, a1, b1) so a
    /// (0, 1, 2), (0, 2, 3) split keeps the triangle strip's diagonal
    /// (a = row j, b = row j + 1; strip order a0 b0 a1 b1 …).
    /// The same grid drawn again as the underglow (0x622D66: second pass,
    /// texture `%s_NEON`, render info +0x344, colour +0x354).
    pub fn neon(&self, color: [u8; 4]) -> Shadow {
        Shadow { color, ..self.clone() }
    }

    pub fn quads(&self) -> Vec<[ShadowVertex; 4]> {
        if let Some(q) = &self.refined {
            return q.clone();
        }
        let mut out = Vec::new();
        for j in (0..3).filter(|&j| self.strips[j]) {
            for c in 0..3 {
                let a = |c: usize| self.vertices[j * 4 + c];
                let b = |c: usize| self.vertices[(j + 1) * 4 + c];
                out.push([b(c), a(c), a(c + 1), b(c + 1)]);
            }
        }
        out
    }
}

/// The ground query's answer for the shadow (0x5E7E80 → 0x5E72F0):
/// height, normal and whether a floor was found. A miss returns the
/// point's own z and the normal (0, 0, −1).
pub type Ground<'a> = dyn FnMut(Vec3) -> Option<(f32, Vec3)> + 'a;

fn query(ground: &mut Ground, p: Vec3) -> (f32, Vec3, bool) {
    match ground(p) {
        Some((z, n)) => (z, n, true),
        None => (p.z, Vec3::new(0.0, 0.0, -1.0), false),
    }
}

/// Build a car's shadow.
///
/// * `transform`: car space → world (the car draw's matrix: orientation
///   with the position as translation).
/// * `box_`: the car's box (`Car::shadow_box`).
/// * `wheels`: the car type's wheel positions, car space.
/// * `ground`: `Some` in game, where every vertex is put on the ground
///   (the branch taken when the car has a collision context, render info
///   [0] + 0x5C4, and the game is not in the front end, 0x8654A4 ≠ 3);
///   `None` in the front end, where the grid lies flat at 0.152 m.
///
/// Returns `None` when nothing is drawn (alpha 0).
pub fn car_shadow(
    transform: Mat4,
    box_: (Vec3, Vec3),
    wheels: [Vec3; 4],
    ground: Option<&mut Ground>,
) -> Option<Shadow> {
    if box_.0.cmpgt(box_.1).any() {
        return None; // no models
    }
    let frontend = ground.is_none();
    let mut ground = ground;
    let position = transform.w_axis.truncate();
    let (x_axis, y_axis) = (transform.x_axis.truncate(), transform.y_axis.truncate());

    // Height above the ground at the car's position, as 0..1 over 0..3 m
    // (0x7A6668 / 0x7A666C, clamped by 0x401090); 0 in the front end.
    // (A non-zero 0x8A1BE8 replaces it: not traced.)
    let h = match ground.as_deref_mut() {
        Some(g) => position.z - query(g, position).0,
        None => 0.0,
    };
    let t = if h > 0.0 && h < 3.0 {
        h / 3.0
    } else if h > 3.0 {
        1.0
    } else {
        0.0
    };

    // The grid: the box grown with height (1 + 0.75·t, 0x7A6670) and by
    // the per-mode factors across x and y (game 1.05 / 1.05, 0x7A66A0 /
    // 0x7A66A4; front end 1.2 / 1.14, 0x7A66AC / 0x7A66A8); the four inner
    // vertices raised by 0.08 in game (0x7A66B4), 0 in the front end
    // (0x7A66B8).
    let (sx, sy, raise) = if frontend { (1.2, 1.14, 0.0) } else { (1.05, 1.05, 0.08) };
    let s = 1.0 + 0.75 * t;
    let (lo, hi) = box_;
    let (x0, x1) = (lo.x * s * sx, hi.x * s * sx);
    let (y0, y1) = (lo.y * s * sy, hi.y * s * sy);
    let (dx, dy) = ((x1 - x0) * (1.0 / 3.0), (y1 - y0) * (1.0 / 3.0));
    let mut vertices = [ShadowVertex { position: Vec3::ZERO, uv: [0.0; 2] }; 16];
    for row in 0..4 {
        for col in 0..4 {
            let (x, y) = (x0 + dx * col as f32, y0 + dy * row as f32);
            // Only the matrix's x and y rows' x/y parts (its z row times 0).
            let p = position.truncate() + Vec2::new(x * x_axis.x + y * y_axis.x, x * x_axis.y + y * y_axis.y);
            vertices[row * 4 + col] =
                ShadowVertex { position: p.extend(position.z), uv: [col as f32 / 3.0, row as f32 / 3.0] };
        }
    }
    let inner = |k: usize| ((k >> 1) ^ k) & 5 == 5;

    let mut fails = [0u32; 4];
    match ground {
        Some(g) => {
            // The ground at the car (0x8A1F00..08, normal 0x8A2CD0), what
            // each vertex is checked against.
            let (car_z, car_n, _) = query(g, position);
            // Each wheel's ground plane: its world point dropped onto the
            // ground, and the normal scaled by −1/n.z (0x4011F0).
            let planes: [(Vec3, Vec3, bool); 4] = std::array::from_fn(|k| {
                let w = transform.transform_point3(wheels[k]);
                let (z, n, hit) = query(g, w);
                (w.with_z(z), n * (-1.0 / n.z), hit)
            });
            let mut last = position.z;
            for (k, v) in vertices.iter_mut().enumerate() {
                let (mut z, _, mut hit) = query(g, v.position);
                let (at, slope, wheel_hit) = planes[VERTEX_WHEEL[k]];
                if !hit && wheel_hit {
                    hit = true;
                    z = (v.position.x - at.x) * slope.x + (v.position.y - at.y) * slope.y + at.z;
                }
                if inner(k) {
                    z += raise;
                }
                let p = v.position.with_z(z);
                // 0x60CF80: within 4 m of the car's ground; 0x60CFA0:
                // within 1.5 m of its plane.
                let near = (z - car_z).abs() < 4.0 && car_n.dot(p - position.with_z(car_z)).abs() < 1.5;
                if hit && near {
                    last = z;
                } else {
                    fails[k / 4] += 1;
                    z = last;
                }
                v.position.z = z + 0.04; // 0x7A66BC
            }
        }
        None => {
            // 0x617229: flat at 0.152 m (0x7A66C4).
            for (k, v) in vertices.iter_mut().enumerate() {
                v.position.z = 0.152 + if inner(k) { raise } else { 0.0 };
            }
        }
    }

    // Alpha: 2·(base + (top − base)·(1 − t)·|up.z|), truncated (0x75BED4)
    // and clamped to 0..0xFE; base 0 and top 0x72 in game, both 0x80 in
    // the front end.
    let (base, top) = if frontend { (128.0, 128.0) } else { (0.0, 114.0) };
    let up_z = transform.z_axis.z.abs();
    let a = (((top - base) * (1.0 - t) * up_z + base) * 2.0) as i32;
    let alpha = a.clamp(0, 0xFE) as u8;
    if alpha == 0 {
        return None;
    }
    let strips = std::array::from_fn(|j| fails[j] + fails[j + 1] < 4);
    let fade = transform.z_axis.z.clamp(0.0, 1.0) * (1.0 - t);
    Some(Shadow { vertices, strips, alpha, color: [0x80, 0x80, 0x80, alpha], fade, refined: None })
}

#[cfg(test)]
mod tests {
    use super::*;

    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/fixtures/shadow-refine-reference.rs"));

    #[test]
    fn refinement_matches_reference_and_stateful_query_order() {
        let original = car_shadow(Mat4::IDENTITY, BOX, WHEELS, None).unwrap();
        for n in [1, 2, 3, 4, 5] {
            for mask in 0..8 {
                let mut input = original.clone();
                input.strips = std::array::from_fn(|i| mask & (1 << i) != 0);
                let run = |optimized| {
                    let mut calls = Vec::new();
                    let mut ground = |p: Vec3| {
                        calls.push(p.to_array().map(f32::to_bits));
                        // Cache-order-sensitive answer, misses and rejected
                        // height jumps: changing query order changes output.
                        match calls.len() % 11 {
                            0 => None,
                            1 => Some((p.z + 2.0, Vec3::Z)),
                            _ => Some((p.x * 0.03 + p.y * 0.05 + calls.len() as f32 * 0.001, Vec3::Z)),
                        }
                    };
                    let output = if optimized {
                        input.clone().refine(n, &mut ground)
                    } else {
                        reference(input.clone(), n, &mut ground)
                    };
                    let bits: Vec<_> = output
                        .quads()
                        .iter()
                        .flatten()
                        .map(|v| (v.position.to_array().map(f32::to_bits), v.uv.map(f32::to_bits)))
                        .collect();
                    (calls, bits, output)
                };
                let (rc, rb, r) = run(false);
                let (ac, ab, a) = run(true);
                assert_eq!(ac, rc, "query order n={n} mask={mask}");
                assert_eq!(ab, rb, "vertex bits n={n} mask={mask}");
                assert_eq!(
                    (a.vertices, a.strips, a.alpha, a.color, a.fade.to_bits()),
                    (r.vertices, r.strips, r.alpha, r.color, r.fade.to_bits())
                );
                assert_eq!(
                    a.toward_eye(Vec3::splat(5.0), Vec3::ZERO, SHADOW_PULL).neon([1, 2, 3, 4]).quads(),
                    r.toward_eye(Vec3::splat(5.0), Vec3::ZERO, SHADOW_PULL).neon([1, 2, 3, 4]).quads()
                );
            }
        }
    }

    #[test]
    fn refined_shadow_keeps_the_native_vertices_and_follows_the_ground() {
        // A kerb: 0.15 m higher for x > 0.5.
        let kerb = |p: Vec3| Some((if p.x > 0.5 { 0.15 } else { 0.0 }, Vec3::Z));
        let mut g = kerb;
        let s = car_shadow(Mat4::IDENTITY, BOX, WHEELS, Some(&mut g)).unwrap();
        let native = s.quads();
        let mut g = kerb;
        let r = s.clone().refine(1, &mut g);
        assert_eq!(r.quads(), native, "n = 1 is native");
        let mut g = kerb;
        let r = s.clone().refine(4, &mut g);
        let q = r.quads();
        assert_eq!(q.len(), native.len() * 16);
        // Every native vertex is still there, unmoved.
        for v in native.iter().flatten() {
            assert!(q.iter().flatten().any(|w| (w.position - v.position).length() < 1e-5 && w.uv == v.uv), "{v:?}");
        }
        // Between them, the vertices sit on their own ground.
        for w in q.iter().flatten() {
            let ground = if w.position.x > 0.5 { 0.15 } else { 0.0 };
            assert!(w.position.z >= ground + 0.04 - 1e-4, "{w:?}");
        }
    }

    const BOX: (Vec3, Vec3) = (Vec3::new(-2.0, -1.0, 0.0), Vec3::new(2.0, 1.0, 1.3));
    const WHEELS: [Vec3; 4] =
        [Vec3::new(1.3, 0.8, 0.3), Vec3::new(1.3, -0.8, 0.3), Vec3::new(-1.3, -0.8, 0.3), Vec3::new(-1.3, 0.8, 0.3)];

    #[test]
    fn frontend_grid_is_flat_and_nearly_opaque() {
        let s = car_shadow(Mat4::IDENTITY, BOX, WHEELS, None).unwrap();
        assert_eq!(s.alpha, 0xFE);
        assert!(s.vertices.iter().all(|v| v.position.z == 0.152));
        assert_eq!(s.vertices[0].position.truncate(), Vec2::new(-2.4, -1.14));
        assert_eq!(s.vertices[15].position.truncate(), Vec2::new(2.4, 1.14));
        assert_eq!(s.vertices[6].uv, [2.0 / 3.0, 1.0 / 3.0]);
        assert_eq!(s.quads().len(), 9);
    }

    #[test]
    fn game_grid_sits_on_the_ground() {
        let at = Mat4::from_translation(Vec3::new(10.0, 20.0, 5.3));
        let mut flat = |_p: Vec3| Some((5.0, Vec3::Z));
        let s = car_shadow(at, BOX, WHEELS, Some(&mut flat)).unwrap();
        // 0.3 m up: t = 0.1, alpha = trunc(2·114·0.9).
        assert_eq!(s.alpha, 205);
        assert!((s.vertices[0].position.z - 5.04).abs() < 1e-5);
        assert!((s.vertices[5].position.z - 5.12).abs() < 1e-5);
        assert_eq!(s.strips, [true; 3]);
        let half = 2.0 * 1.075 * 1.05;
        assert!((s.vertices[3].position.x - (10.0 + half)).abs() < 1e-4);
    }

    #[test]
    fn misses_fall_back_to_the_wheel_planes_then_skip_strips() {
        let at = Mat4::from_translation(Vec3::new(0.0, 0.0, 0.3));
        // Ground only right under the car's centre and wheels.
        let mut sparse =
            |p: Vec3| (p.truncate().length() < 0.1 || (p.x.abs() - 1.3).abs() < 0.01).then_some((0.0, Vec3::Z));
        let s = car_shadow(at, BOX, WHEELS, Some(&mut sparse)).unwrap();
        assert_eq!(s.strips, [true; 3]);
        assert!((s.vertices[0].position.z - 0.04).abs() < 1e-5);
        let mut none = |_p: Vec3| None;
        let s = car_shadow(at, BOX, WHEELS, Some(&mut none)).unwrap();
        assert_eq!(s.strips, [false; 3]);
    }

    #[test]
    fn neon_tubes_face_the_camera_and_fade_with_distance() {
        // Camera 5 m behind on −y looking along +y: right +x, down −z.
        let view = |eye: Vec3| TubeView {
            eye,
            right: Vec3::X,
            down: -Vec3::Z,
            view_projection: Mat4::perspective_rh(1.0, 4.0 / 3.0, 0.5, 1000.0)
                * Mat4::look_to_rh(eye, Vec3::Y, Vec3::Z),
            aspect: 4.0 / 3.0,
            fov: 13018,
        };
        let tubes = [[Vec3::new(-1.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)]];
        let mut q = Vec::new();
        neon_tube_quads(
            &view(Vec3::new(0.0, -5.0, 0.0)),
            &tubes,
            Vec3::ZERO,
            Mat4::IDENTITY,
            [220, 220, 220, 40],
            0.045,
            true,
            1.0,
            7,
            &mut q,
        );
        assert_eq!(q.len(), 4);
        assert_eq!(q[0].rgba, [220, 220, 220, 80]);
        // A horizontal tube: the ribbon spans ±0.045 m in z, the caps
        // reach 0.045 m past the ends in x.
        assert!((q[0].corners[2] - Vec3::new(1.0, 0.0, -0.045)).length() < 1e-4, "{:?}", q[0].corners);
        assert!((q[1].corners[2] - Vec3::new(1.0, 0.0, 0.045)).length() < 1e-4);
        assert!((q[2].corners[1].x + 1.045).abs() < 1e-4 && (q[3].corners[2].x - 1.045).abs() < 1e-4);
        // Half way from 10 m to 75 m: width ×10.5, alpha halved.
        let mut far = Vec::new();
        neon_tube_quads(
            &view(Vec3::new(0.0, -42.5, 0.0)),
            &tubes,
            Vec3::ZERO,
            Mat4::IDENTITY,
            [220, 220, 220, 40],
            0.045,
            false,
            1.0,
            7,
            &mut far,
        );
        assert_eq!(far[0].rgba[3], 40);
        assert!((far[0].corners[2].z + 0.045 * 10.5).abs() < 1e-3);
        // Beyond 75 m: nothing.
        let mut none = Vec::new();
        neon_tube_quads(
            &view(Vec3::new(0.0, -80.0, 0.0)),
            &tubes,
            Vec3::ZERO,
            Mat4::IDENTITY,
            [220, 220, 220, 40],
            0.045,
            false,
            1.0,
            7,
            &mut none,
        );
        assert!(none.is_empty());
        // Unlit neon: +0x358 stays 0.
        assert_eq!(neon_tube_color(&Neon::default(), 1.0), [0; 4]);
    }

    #[test]
    fn neon_colour_and_pulse() {
        // MAGENTA: steady; alpha trunc(64 · fade) · 4.
        let magenta = Neon { base: [230, 26, 189, 64], delta: [0; 4], lit: true };
        assert_eq!(neon_color(&magenta, 0.0, 1.0), Some([189, 26, 230, 0xFF]));
        assert_eq!(neon_color(&magenta, 3.0, 0.5), Some([189, 26, 230, 128]));
        assert_eq!(neon_color(&magenta, 0.0, 0.0), None);
        assert_eq!(neon_color(&Neon { lit: false, ..magenta }, 0.0, 1.0), None);
        // RED PULSE: ALPHA 16 → ALPHA2 64, by |sin 4.7 s|.
        let pulse = Neon { base: [128, 0, 0, 16], delta: [0, 0, 0, 48], lit: true };
        assert_eq!(neon_color(&pulse, 0.0, 0.25), Some([0, 0, 128, 16]));
        let peak = std::f32::consts::FRAC_PI_2 / 4.7;
        let c = neon_color(&pulse, peak, 0.25).unwrap();
        assert!((60..=64).contains(&c[3]), "{c:?}");
        // The light colour is R, G, B (MAGENTA's red over its blue; a
        // yellow neon stays yellow, never blue).
        let rgb = neon_rgb(&magenta, 0.0).unwrap();
        assert!((rgb.x - 230.0 / 255.0).abs() < 1e-4 && (rgb.z - 189.0 / 255.0).abs() < 1e-4, "{rgb:?}");
        let yellow = Neon { base: [240, 200, 20, 64], delta: [0; 4], lit: true };
        let y = neon_rgb(&yellow, 0.0).unwrap();
        assert!(y.x > 0.9 && y.y > 0.7 && y.z < 0.1, "{y:?}");
        assert_eq!(neon_rgb(&Neon { lit: false, ..yellow }, 0.0), None);
        // Channels clamp at 0xFE.
        let bright = Neon { base: [250, 0, 0, 250], delta: [100, 0, 0, 0], lit: true };
        assert_eq!(neon_color(&bright, peak, 1.0).unwrap()[2], 0xFE);
    }
}

#[cfg(test)]
mod pull_tests {
    use super::*;

    #[test]
    fn pull_by_camera_mover() {
        assert_eq!(shadow_pull(None), 0.104);
        assert_eq!(shadow_pull(Some(1)), 0.104);
        assert_eq!(shadow_pull(Some(0x12)), 0.023);
        assert_eq!(shadow_pull(Some(0x14)), 0.023);
    }

    /// The underglow's light on the ground is its brightest channel
    /// × its drawn alpha (MAGENTA at full alpha ≈ 1; no alpha, none).
    #[test]
    fn neon_light_follows_peak_channel_and_alpha() {
        assert!((neon_light([0xFE, 0x00, 0xFE, 0xFF]) - 254.0 / 255.0).abs() < 1e-6);
        assert_eq!(neon_light([0xFE, 0xFE, 0xFE, 0x00]), 0.0);
        assert!((neon_light([0x00, 0x80, 0x00, 0x80]) - (128.0 / 255.0) * (128.0 / 255.0)).abs() < 1e-6);
    }
}
