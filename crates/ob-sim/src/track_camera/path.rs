use glam::{Mat4, Vec3};

use super::route_director::chains;
use crate::formats::camera_tracks::{CameraEntry, CameraTrack};
use crate::draw::camera::Camera;

/// One coordinate of a path (0x446180 / 0x446210): a Hermite curve from the
/// entry's two points with the end tangents `m` ([`end_tangents`], 0x450040),
/// or a straight lerp.
pub(super) fn path(points: [[f32; 3]; 2], m: [Vec3; 2], spline: bool, t: f32) -> Vec3 {
    let (p0, p1) = (Vec3::from(points[0]), Vec3::from(points[1]));
    if !spline {
        return p0.lerp(p1, t);
    }
    let (m0, m1) = (m[0], m[1]);
    let (t2, t3) = (t * t, t * t * t);
    p0 * (2.0 * t3 - 3.0 * t2 + 1.0) + m0 * (t3 - 2.0 * t2 + t) + p1 * (-2.0 * t3 + 3.0 * t2) + m1 * (t3 - t2)
}

/// An entry's share of its track's fraction (0x442D70, **verified**: the
/// next entry's start less its own, 0x442D00; past the last entry 1.0).
pub(super) fn fraction_length(track: &CameraTrack, i: usize) -> f32 {
    let start = |j: usize| track.entries.get(j).map_or(1.0, |e| e.start);
    start(i + 1) - start(i)
}

/// The tangents of entry `i` at its two ends, for the eye and look paths
/// and the lens (0x450040, **verified** in the decompile; s = the entry's
/// tangent scale +0x10 / +0x14 for that end). An entry of type 0 has none.
/// Unchained: (p1 − p0) · s. Chained to its neighbour n at that end (the
/// previous entry at the start, the next at the end; [`chains`]): with dt,
/// dn the two entries' fraction lengths ([`fraction_length`]), w = dt /
/// (dn + dt), B = s · (1 − w), A = s · w · dt / dn (s · w when dn ≤ 1e-6),
/// the tangent is A · (n.p1 − n.p0) + B · (p1 − p0).
pub(super) fn end_tangents(track: &CameraTrack, i: usize) -> ([Vec3; 2], [Vec3; 2], [f32; 2]) {
    let e = &track.entries[i];
    let mut out = ([Vec3::ZERO; 2], [Vec3::ZERO; 2], [0.0f32; 2]);
    if e.mover == 0 {
        return out;
    }
    let d = |p: [[f32; 3]; 2]| Vec3::from(p[1]) - Vec3::from(p[0]);
    for end in 0..2 {
        let s = e.tangent[end];
        let n = if end == 0 { i.checked_sub(1) } else { Some(i + 1).filter(|&j| j < track.entries.len()) };
        let chained = n.filter(|&n| {
            let ne = &track.entries[n];
            ne.mover != 0 && if end == 0 { chains(ne, e) } else { chains(e, ne) }
        });
        if let Some(n) = chained {
            let ne = &track.entries[n];
            let (dt, dn) = (fraction_length(track, i), fraction_length(track, n));
            let w = dt / (dn + dt);
            let b = s * (1.0 - w);
            let a = s * if dn > 1e-6 { w / dn * dt } else { w };
            out.0[end] = d(ne.eye) * a + d(e.eye) * b;
            out.1[end] = d(ne.look) * a + d(e.look) * b;
            out.2[end] = (ne.fov[1] - ne.fov[0]) * a + (e.fov[1] - e.fov[0]) * b;
        } else {
            out.0[end] = d(e.eye) * s;
            out.1[end] = d(e.look) * s;
            out.2[end] = (e.fov[1] - e.fov[0]) * s;
        }
    }
    out
}

/// A point of a space (+4 / +5): 0 the car's frame, 1 world, 2 world axes
/// about the car's position, 3 the scripted scene's frame (scene +0xE0,
/// the origin 0x43B000 builds with 0x431990; read as world without a
/// scene). A space-3 point first takes the ground step (0x447A30 /
/// 0x447A80 → 0x4427C0, ECX = 0x828770, **verified**): its z += the ground
/// under the point in the scene's frame (0x441000 on scene +0xE0) less the
/// ground under the frame's origin (+0x110, kept in +0x1C60), each a
/// 0x5E6FB0(p, 1.0) / 0x5E72F0 query (0x442700); nothing when no ground
// prov: inferred track_camera.to_world
/// is under the point. `ground` is that query (**inferred**: the clone's
/// highest walkable surface below 1 m over the point; with no ground
/// under the origin the step is left out).
pub(super) fn to_world(
    p: Vec3,
    space: u8,
    car: Mat4,
    scene: Option<Mat4>,
    ground: Option<&dyn Fn(Vec3) -> Option<f32>>,
) -> Vec3 {
    match (space, scene) {
        (0, _) => car.transform_point3(p),
        (2, _) => p + car.w_axis.truncate(),
        (3, Some(m)) => {
            let step = ground.and_then(|g| Some(g(m.transform_point3(p))? - g(m.w_axis.truncate())?)).unwrap_or(0.0);
            m.transform_point3(p + Vec3::Z * step)
        }
        _ => p,
    }
}

/// The track camera's view at a fraction of an entry (0x453F50): the eye
/// and look paths in their spaces, up +z, the lens ([`lens_angle`])
/// lerped (**inferred** for a spline entry, which the game takes from the
/// curve 0x574880). None when the eye and look coincide.
pub fn camera(track: &CameraTrack, i: usize, fraction: f32, car: Mat4) -> Option<Camera> {
    camera_in(track, i, fraction, car, None, None)
}

/// [`camera`] with the scripted scene's frame for space-3 points.
pub fn camera_in(
    track: &CameraTrack,
    i: usize,
    fraction: f32,
    car: Mat4,
    scene: Option<Mat4>,
    ground: Option<&dyn Fn(Vec3) -> Option<f32>>,
) -> Option<Camera> {
    let e = &track.entries[i];
    let end = track.entries.get(i + 1).map_or(1.0, |n| n.start);
    let t = if end > e.start { ((fraction - e.start) / (end - e.start)).clamp(0.0, 1.0) } else { 0.0 };
    let (eye_m, look_m, _) = end_tangents(track, i);
    let eye = to_world(path(e.eye, eye_m, e.spline, t), e.eye_space, car, scene, ground);
    let look = to_world(path(e.look, look_m, e.spline, t), e.look_space, car, scene, ground);
    if (look - eye).length_squared() < 1e-8 || !eye.is_finite() || !look.is_finite() {
        return None;
    }
    let fov = e.fov[0] + (e.fov[1] - e.fov[0]) * t;
    let mut camera = Camera::look_at(eye, look, Vec3::Z, fov.to_radians());
    camera.game_fov = Some(lens_angle(fov));
    Some(camera)
}

/// The letterbox at `fraction` in entry `i`: each black bar's share of the
/// screen height (**verified**, 0x451B70 / 0x453F50): the entry's bytes
/// +0x72 / +0x73 (START_LETTERBOX / END_LETTERBOX of the debug print at
/// 0x787800) are the start and end of the mover's channel +0x50 (lerped
/// like the lens), and the mover writes the channel × 0.125 (0x7870A4) ×
/// 0.01 (0x78425C) to its camera's +0xC8 (0x454553..0x45456C): 100, the
/// race start scenes' value, is 0.125 of the height a bar — **observed**
/// under Wine as 120-pixel bars top and bottom at 1280 × 960 (Quick Race
/// circuit and sprint starts, race112). Shop and ending scenes carry 0
/// (no bars); CareerNis1 eases 89 → 74. The routine that draws the bars
/// from camera +0xC8 is **not traced** (the clone draws two black
/// rectangles over the 3D view, under nothing else).
pub fn letterbox(track: &CameraTrack, i: usize, fraction: f32) -> f32 {
    let e = &track.entries[i];
    let end = track.entries.get(i + 1).map_or(1.0, |n| n.start);
    let t = if end > e.start { ((fraction - e.start) / (end - e.start)).clamp(0.0, 1.0) } else { 0.0 };
    let (a, b) = (f32::from(e.tail[2]), f32::from(e.tail[3]));
    (a + (b - a) * t) * 0.125 * 0.01
}

/// The entry's +0x48 / +0x4C are a lens's focal length (mm), not an
/// angle: the track mover writes view 1's camera +0xC4 as the binary angle
/// 2·atan(LENS_HALF_WIDTH / f) (the lerped curve, 0x4462A0 → 0x75BED4 at
/// 0x454111), and 0x5C7ED0 projects it widened ×1.1 (`Camera::game_fov`).
/// **Measured** in the original under Wine (2026-10-29, CareerNis1 with
/// the peek channel): f 50.031 → 6440, 29.971 → 10206, 24.046 → 12222
/// (35.4°, 56.1°, 67.1°): a half width of 15.96 mm to 0.1 % (16 mm, a
/// 32 mm gate, is 0.2 % wider). The conversion's code is **not traced**.
pub const LENS_HALF_WIDTH: f32 = 15.965;

/// A focal length's angle (radians), as camera +0xC4 holds it.
pub fn lens_angle(focal: f32) -> f32 {
    2.0 * (LENS_HALF_WIDTH / focal.max(1e-3)).atan()
}

/// The director's track state: the shot held while the car stays on its
/// group, and the picking each frame it has none.
/// The view showing when a track is drawn, as 0x448280 reads it from the
/// view's camera (0x48B1E0(+0x18) +0x40): its eye (+0x40), the direction
/// to its look point (+0x60 − +0x40), its field of view (+0xC4, radians)
/// and its velocity (+0x1E0: the eye's (position − last) / dt).
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub eye: Vec3,
    pub forward: Vec3,
    pub fov: f32,
    pub velocity: Vec3,
}

/// The visibility test 0x448280 (**verified** in the disassembly) on a
/// candidate track's entry at the car's fraction, as the route draw
/// 0x44F9A0 asks it (and the typed draw under a weight of 10):
///
/// * The entry's eye start (0x447A30(0): +0x18, its y negated under the
///   flag 0x828649, the ground step of space 3) is taken through the car's
///   frame (0x401340 of car +0x90 / +0x60) **whatever its space** — so
///   for the route cameras' world-space eyes (space 1, most of L4RA's)
///   this point is not where the shot's eye will be.
/// * The sight test 0x5E7EE0(car +0x60, that point): the segment against
///   the world's polygons gathered around it (0x5E6FB0, by 30 m pieces
///   when longer) and 0x5D3930's crossing test (0x5D3800, the box grown by
///   0.5) — any crossing fails. Then 0x5F1500(point, 0.5, car): a car of
///   the world list (not this one, active +0x550) whose centre is within
///   √5.5 m and whose body box (+0xE0 / +0xF0) grown by 0.5 holds the
///   point fails ([`eye_in_car`]).
/// * Against the view showing, in the car's frame: d = its look
///   direction, e = the entry's (look start − eye start, 0x447A80(0)),
///   both normalised: e.y·d.y < 0 fails (the other side of the car); e·d
///   ≤ −0.5 fails; e·d ≥ 0.866: the car's apparent sizes 1 / (|eye| ·
///   tan(fov / 2)) of the view (its eye in the car's frame, +0xC4 / 2)
///   and of the entry (|eye start|, the entry's +0x48 through 0x443D20)
///   must differ by more than half the larger, then the motion test;
///   0 < e·d < 0.866: the motion test; e·d ≤ 0 passes. The motion test:
///   the view's velocity taken through the inverse car frame **as a
///   point** (translation included, 0x4484DC), normalised, against the
///   entry's eye path (eye end − eye start, 0x4011B0), normalised: a dot
///   above 0 passes.
///
/// `blocked(from, to)` is the sight test; the native test also fails on
/// the car test, which the caller folds in with [`eye_in_car`] on `to`.
pub fn visible(e: &CameraEntry, car: Mat4, view: &View, blocked: &mut dyn FnMut(Vec3, Vec3) -> bool) -> bool {
    let eye0 = Vec3::from(e.eye[0]);
    let look0 = Vec3::from(e.look[0]);
    let eye1 = Vec3::from(e.eye[1]);
    if blocked(car.w_axis.truncate(), car.transform_point3(eye0)) {
        return false;
    }
    let inv = car.inverse();
    let d = inv.transform_vector3(view.forward).normalize_or_zero();
    let ed = (look0 - eye0).normalize_or_zero();
    if ed.y * d.y < 0.0 {
        return false;
    }
    let dot = ed.dot(d);
    if dot <= -0.5 {
        return false;
    }
    if dot >= 0.866 {
        let near = 1.0 / (inv.transform_point3(view.eye).length() * (view.fov * 0.5).tan());
        let far = 1.0 / (eye0.length() * (lens_angle(e.fov[0]) * 0.5).tan());
        if (far - near).abs() / near.max(far) <= 0.5 {
            return false;
        }
    } else if dot <= 0.0 {
        return true;
    }
    let v = inv.transform_point3(view.velocity).normalize_or_zero();
    v.dot((eye1 - eye0).normalize_or_zero()) > 0.0
}

/// The car test of the visibility test (0x5F1500(point, 0.5, car), called
/// by 0x448280 after the sight test; **verified**): any car of the world
/// list other than the director's (active, +0x550) whose origin (+0x50) is
/// within √5.5 m of the point (squared distance < 0.5 + 5.0) and whose
/// body box (+0x3C → +0xE0 / +0xF0) grown by 0.5 on every side holds the
/// point taken into its frame (rows +0x90, origin +0x50; 0x43D7A0, bounds
/// inclusive). `cars` is that list without the director's car.
pub fn eye_in_car(point: Vec3, cars: &[crate::chase::LiftCar]) -> bool {
    cars.iter().any(|c| {
        if !((point - c.origin).length_squared() < 0.5 + 5.0) {
            return false;
        }
        let local = c.rotation.transpose() * (point - c.origin);
        let (lo, hi) = (c.min - 0.5, c.max + 0.5);
        local.cmpge(lo).all() && local.cmple(hi).all()
    })
}
