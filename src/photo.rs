//! Photo mode (docs/playing/photo-mode.md). **Clone deviation**, opt-in: the
//! Enhancements row "Photo Mode in Pause Menu" (default Off) lists "Photo
//! Mode" in the pause menu; nothing of it is in SPEED2.EXE and nothing was
//! traced for it. The world stays paused under it (the pause menu's own
//! pause: no simulation step changes anything, particles and rain hold),
//! and the game's camera is left as it was: photo mode draws from its own
//! camera and puts the game's back after each frame.
//!
//! This module is the logic without the GPU: the camera rig and its limits
//! (a radius around the car, a sphere swept against the world's collision
//! triangles so the eye never ends inside a wall or under the road), the
//! settings strip (field of view, roll, focus, aperture, exposure, filter,
//! bleach, car shadow, light glow, speed blur, picture size), the depth of field's
//! focus band for a focus distance and an f-number, the capture's size
//! guard and the picture's file name.

use crate::render::camera::Camera;
use glam::{Quat, Vec3};

/// The eye stays within this distance (m) of the car's centre.
pub const MAX_RADIUS: f32 = 15.0;
/// The orbit's centre (the point looked at) stays within this distance (m)
/// of the car's centre.
pub const PIVOT_RADIUS: f32 = 10.0;
/// The eye's sphere (m): it keeps this far from every collision face.
pub const EYE_RADIUS: f32 = 0.3;
/// The orbit's distance range (m).
pub const MIN_DISTANCE: f32 = 0.8;
pub const MAX_DISTANCE: f32 = 12.0;
/// The orbit's pitch limit (radians either way).
pub const MAX_PITCH: f32 = 1.45;
/// Vertical field of view range (degrees).
pub const FOV_RANGE: (f32, f32) = (10.0, 100.0);
/// Roll range (degrees either way).
pub const MAX_ROLL: f32 = 45.0;
/// Speeds of the held controls: m/s, rad/s, distance ×/s.
pub const MOVE_SPEED: f32 = 4.0;
pub const TURN_SPEED: f32 = 1.6;
pub const DOLLY_RATE: f32 = 1.8;
/// The f-numbers of the Aperture row (0 = depth of field off).
pub const APERTURES: [f32; 9] = [0.0, 1.4, 2.0, 2.8, 4.0, 5.6, 8.0, 11.0, 16.0];
/// The Focus row's distances (m; 0 = Auto: the car looked at).
pub const FOCUS_STEPS: [f32; 15] = [0.0, 1.0, 1.5, 2.0, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0, 15.0, 20.0, 30.0, 50.0, 100.0];
/// The Exposure row's range (EV) and step.
pub const EXPOSURE_RANGE: (f32, f32) = (-3.0, 3.0);
pub const EXPOSURE_STEP: f32 = 0.25;
/// The Bleach row's steps (%).
pub const BLEACH_STEPS: [u8; 6] = [0, 20, 40, 60, 80, 100];
/// The Capture row's scales (× the display in each direction).
pub const CAPTURE_SCALES: [u32; 4] = [1, 2, 3, 4];
/// The capture's budgets: scene pixels × samples at most 64 × 3840 × 2160
/// (the SSAA rows' coverage budget, docs/notes/graphics/ssaa.md).
pub const CAPTURE_SAMPLE_BUDGET: u64 = 64 * 3840 * 2160;

/// The colour filter over the game's own grade.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Filter {
    /// The game's grade alone.
    #[default]
    Original,
    BlackWhite,
    Sepia,
}

impl Filter {
    pub const ALL: [Filter; 3] = [Filter::Original, Filter::BlackWhite, Filter::Sepia];
    pub fn name(self) -> &'static str {
        match self {
            Filter::Original => "Original",
            Filter::BlackWhite => "Black & White",
            Filter::Sepia => "Sepia",
        }
    }
    /// The shader's filter id.
    pub fn id(self) -> u32 {
        self as u32
    }
}

/// The settings strip's rows, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    Fov,
    Roll,
    Focus,
    Aperture,
    Exposure,
    Filter,
    Bleach,
    CarShadow,
    LightGlow,
    SpeedBlur,
    Capture,
}

impl Row {
    pub const ALL: [Row; 11] = [Row::Fov, Row::Roll, Row::Focus, Row::Aperture, Row::Exposure, Row::Filter, Row::Bleach, Row::CarShadow, Row::LightGlow, Row::SpeedBlur, Row::Capture];
    pub fn name(self) -> &'static str {
        match self {
            Row::Fov => "Field Of View",
            Row::Roll => "Roll",
            Row::Focus => "Focus",
            Row::Aperture => "Aperture",
            Row::Exposure => "Exposure",
            Row::Filter => "Filter",
            Row::Bleach => "Bleach",
            Row::CarShadow => "Car Shadow",
            Row::LightGlow => "Light Glow",
            Row::SpeedBlur => "Speed Blur",
            Row::Capture => "Picture Size",
        }
    }
}

/// The look of the picture: everything but the camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// Focus distance (m), 0 = Auto.
    pub focus: f32,
    /// f-number, 0 = no depth of field.
    pub aperture: f32,
    /// EV.
    pub exposure: f32,
    pub filter: Filter,
    /// %.
    pub bleach: u8,
    pub car_shadow: bool,
    pub light_glow: bool,
    /// The game's own speed blur (Customize Display's Motion Blur, as the
    /// pause shows it).
    pub speed_blur: bool,
    /// × the display.
    pub capture: u32,
}

impl Default for Look {
    fn default() -> Look {
        Look { focus: 0.0, aperture: 0.0, exposure: 0.0, filter: Filter::Original, bleach: 0, car_shadow: true, light_glow: true, speed_blur: true, capture: 2 }
    }
}

impl Look {
    /// The linear gain of the exposure.
    pub fn gain(&self) -> f32 {
        self.exposure.exp2()
    }

    /// Whether the grade pass changes nothing (exposure 0, no filter, no
    /// bleach).
    pub fn grade_is_identity(&self) -> bool {
        self.exposure == 0.0 && self.filter == Filter::Original && self.bleach == 0
    }
}

/// The camera rig: an orbit about a point near the car.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rig {
    /// The point looked at, relative to the car's centre (m, world axes).
    pub pan: Vec3,
    /// The eye's direction from the point (radians): heading about +Z
    /// (0 = the eye on +Y of the point) and elevation.
    pub yaw: f32,
    pub pitch: f32,
    /// m.
    pub distance: f32,
    /// degrees, positive leans the horizon clockwise.
    pub roll: f32,
    /// Vertical field of view, degrees.
    pub fov: f32,
}

impl Rig {
    /// The rig that sees the car's centre `centre` from where `camera` is
    /// (its distance and direction, held to the limits), with its field of
    /// view (`aspect` for a game camera's).
    pub fn from_camera(camera: &Camera, centre: Vec3, aspect: f32) -> Rig {
        let d = camera.position - centre;
        let distance = d.length().clamp(MIN_DISTANCE, MAX_DISTANCE);
        let dir = d.normalize_or(Vec3::new(0.0, -1.0, 0.3).normalize());
        let (_, m11) = camera.projection_scale(aspect);
        let fov = (2.0 * (1.0 / m11).atan()).to_degrees().clamp(FOV_RANGE.0, FOV_RANGE.1);
        Rig { pan: Vec3::ZERO, yaw: dir.x.atan2(dir.y), pitch: dir.z.clamp(-1.0, 1.0).asin().clamp(-MAX_PITCH, MAX_PITCH), distance, roll: 0.0, fov }
    }

    /// Rate independence: the rig a frame `t` of the way from
    /// the tick before (`a`) to the newest (`b`) draws from: it moves on the
    /// ticks (the held keys, the sticks, the drag); exactly `a` at t ≤ 0,
    /// exactly `b` at t ≥ 1, each value within the two between.
    pub fn between(a: &Rig, b: &Rig, t: f32) -> Rig {
        use crate::render::native::fx::tick::lerp;
        if !(t > 0.0) {
            return *a;
        }
        if t >= 1.0 {
            return *b;
        }
        Rig {
            pan: Vec3::new(lerp(a.pan.x, b.pan.x, t), lerp(a.pan.y, b.pan.y, t), lerp(a.pan.z, b.pan.z, t)),
            yaw: lerp(a.yaw, b.yaw, t),
            pitch: lerp(a.pitch, b.pitch, t),
            distance: lerp(a.distance, b.distance, t),
            roll: lerp(a.roll, b.roll, t),
            fov: lerp(a.fov, b.fov, t),
        }
    }

    /// The eye's direction from the point looked at.
    pub fn direction(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        Vec3::new(sy * cp, cy * cp, sp)
    }

    /// The camera's horizontal forward and right (for the move keys).
    pub fn ground_axes(&self) -> (Vec3, Vec3) {
        let f = -Vec3::new(self.direction().x, self.direction().y, 0.0).normalize_or(Vec3::Y);
        (f, f.cross(Vec3::Z).normalize_or(Vec3::X))
    }

    /// One frame of held controls: `mv` (forward, right, up) in −1..1,
    /// `turn` (yaw, pitch) in −1..1, `dolly` > 0 nearer, `dt` s.
    pub fn drive(&mut self, mv: Vec3, turn: (f32, f32), dolly: f32, dt: f32) {
        let (f, r) = self.ground_axes();
        self.pan += (f * mv.x + r * mv.y + Vec3::Z * mv.z) * MOVE_SPEED * dt;
        self.yaw += turn.0 * TURN_SPEED * dt;
        self.pitch = (self.pitch + turn.1 * TURN_SPEED * dt).clamp(-MAX_PITCH, MAX_PITCH);
        self.distance = (self.distance * (-dolly * DOLLY_RATE * dt).exp()).clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    /// The point looked at and the eye for a car centred at `centre`,
    /// held to the limits: the point within [`PIVOT_RADIUS`] of the centre
    /// and reached from it without crossing a face, the eye within
    /// [`MAX_RADIUS`] and reached from the point without its sphere
    /// touching a face of `faces` ([`sweep`]).
    pub fn place(&self, centre: Vec3, faces: &[[Vec3; 3]]) -> (Vec3, Vec3) {
        let mut pan = self.pan;
        if pan.length() > PIVOT_RADIUS {
            pan = pan.normalize() * PIVOT_RADIUS;
        }
        let pivot = centre + pan * sweep(centre, centre + pan, EYE_RADIUS, faces);
        let mut eye = pivot + self.direction() * self.distance;
        let off = eye - centre;
        if off.length() > MAX_RADIUS {
            eye = centre + off.normalize() * MAX_RADIUS;
        }
        let eye = pivot + (eye - pivot) * sweep(pivot, eye, EYE_RADIUS, faces);
        (pivot, eye)
    }

    /// The camera at `eye` looking at `pivot`, rolled, with the rig's
    /// field of view.
    pub fn camera(&self, pivot: Vec3, eye: Vec3) -> Camera {
        let mut c = Camera::looking_at(eye, if pivot.distance(eye) > 1e-3 { pivot } else { eye - self.direction() });
        c.fov_y = self.fov.to_radians();
        c.game_fov = None;
        c.near = 0.1;
        c.up = Quat::from_axis_angle(c.forward(), self.roll.to_radians()) * Vec3::Z;
        c
    }

    /// The Field Of View and Roll rows: `dir` ±1 one step.
    pub fn step(&mut self, row: Row, dir: i32) {
        match row {
            Row::Fov => self.fov = (self.fov + 5.0 * dir as f32).clamp(FOV_RANGE.0, FOV_RANGE.1),
            Row::Roll => self.roll = (self.roll + 5.0 * dir as f32).clamp(-MAX_ROLL, MAX_ROLL),
            _ => {}
        }
    }
}

/// The largest fraction `t` of `from → to` the sphere of radius `r` moves
/// before it touches a face (steps of half the radius, refined by halving;
/// `1` when it never does). Faces the sphere already touches at `from` (a
/// car parked against a wall) do not hold it: it may move off them.
pub fn sweep(from: Vec3, to: Vec3, r: f32, faces: &[[Vec3; 3]]) -> f32 {
    let len = from.distance(to);
    if len < 1e-5 || faces.is_empty() {
        return 1.0;
    }
    let steps = (len / (r * 0.5)).ceil().max(1.0) as u32;
    let near = |p: Vec3, t: &[Vec3; 3]| closest_on_triangle(p, t).distance_squared(p) < r * r;
    let faces: Vec<&[Vec3; 3]> = faces.iter().filter(|t| !near(from, t)).collect();
    let touches = |p: Vec3| faces.iter().any(|t| near(p, t));
    let mut free = 0.0;
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        if touches(from.lerp(to, t)) {
            // the last free step, refined by halving
            let (mut lo, mut hi) = (free, t);
            for _ in 0..6 {
                let mid = (lo + hi) * 0.5;
                if touches(from.lerp(to, mid)) { hi = mid } else { lo = mid }
            }
            return lo;
        }
        free = t;
    }
    1.0
}

/// The point of triangle `t` nearest `p` (Ericson, Real-Time Collision
/// Detection 5.1.5).
pub fn closest_on_triangle(p: Vec3, t: &[Vec3; 3]) -> Vec3 {
    let [a, b, c] = *t;
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = 1.0 / (va + vb + vc);
    a + ab * (vb * denom) + ac * (vc * denom)
}

/// The depth range (m from the eye) kept sharp when focused at `focus` m
/// with f-number `n`: wider with a larger f-number (×(1 ± n/24), the far
/// side open past the hyperfocal-like point at n/24 ≥ 1).
pub fn focus_band(focus: f32, n: f32) -> (f32, f32) {
    let k = (n / 24.0).clamp(0.02, 1.0);
    let near = (focus * (1.0 - k)).max(0.1);
    let far = if k >= 1.0 { f32::MAX } else { focus * (1.0 + k) / (1.0 - k * 0.5) };
    (near, far)
}

/// The largest circle of confusion as a share of the Cinematic depth of
/// field's for f-number `n` (2.8 the same; f/1.4 twice, f/16 a sixth).
pub fn coc_scale(n: f32) -> f32 {
    if n <= 0.0 { 0.0 } else { (2.8 / n).clamp(0.15, 2.0) }
}

/// The capture's scale: the largest of `requested` and below whose size
/// (`out` × scale) fits `max_dim` on each side and whose pixels × `samples`
/// fit [`CAPTURE_SAMPLE_BUDGET`]; at least 1.
pub fn capture_scale(requested: u32, out: (u32, u32), samples: u32, max_dim: u32) -> u32 {
    let mut k = requested.max(1);
    while k > 1 {
        let (w, h) = (u64::from(out.0) * u64::from(k), u64::from(out.1) * u64::from(k));
        if w <= u64::from(max_dim) && h <= u64::from(max_dim) && w * h * u64::from(samples.max(1)) <= CAPTURE_SAMPLE_BUDGET {
            break;
        }
        k -= 1;
    }
    k
}

/// The photo's file name for the local time `stamp` ("YYYY-MM-DD
/// HH-MM-SS") and a count of photos already taken in that second.
pub fn file_name(stamp: &str, n: u32) -> String {
    if n == 0 { format!("Overbright {stamp}.png") } else { format!("Overbright {stamp} ({}).png", n + 1) }
}

/// One step of `row` of `look` (or of `rig`) by `dir` (±1).
pub fn step(rig: &mut Rig, look: &mut Look, row: Row, dir: i32) {
    fn step_in<T: PartialEq + Copy>(steps: &[T], v: T, dir: i32) -> T {
        let i = steps.iter().position(|s| *s == v).unwrap_or(0) as i32;
        steps[(i + dir).clamp(0, steps.len() as i32 - 1) as usize]
    }
    match row {
        Row::Fov | Row::Roll => rig.step(row, dir),
        Row::Focus => look.focus = step_in(&FOCUS_STEPS, look.focus, dir),
        Row::Aperture => look.aperture = step_in(&APERTURES, look.aperture, dir),
        Row::Exposure => look.exposure = (look.exposure + EXPOSURE_STEP * dir as f32).clamp(EXPOSURE_RANGE.0, EXPOSURE_RANGE.1),
        Row::Filter => look.filter = Filter::ALL[(look.filter as i32 + dir).rem_euclid(3) as usize],
        Row::Bleach => look.bleach = step_in(&BLEACH_STEPS, look.bleach, dir),
        Row::CarShadow => look.car_shadow = !look.car_shadow,
        Row::LightGlow => look.light_glow = !look.light_glow,
        Row::SpeedBlur => look.speed_blur = !look.speed_blur,
        Row::Capture => look.capture = step_in(&CAPTURE_SCALES, look.capture, dir),
    }
}

/// A row's value as the strip shows it.
pub fn value(rig: &Rig, look: &Look, row: Row) -> String {
    let on = |b: bool| if b { "On" } else { "Off" }.to_string();
    match row {
        Row::Fov => format!("{:.0}\u{B0}", rig.fov),
        Row::Roll => format!("{:+.0}\u{B0}", rig.roll),
        Row::Focus => if look.focus == 0.0 { "Auto".into() } else { format!("{} m", look.focus) },
        Row::Aperture => if look.aperture == 0.0 { "Off".into() } else { format!("f/{}", look.aperture) },
        Row::Exposure => format!("{:+.2} EV", look.exposure),
        Row::Filter => look.filter.name().into(),
        Row::Bleach => format!("{}%", look.bleach),
        Row::CarShadow => on(look.car_shadow),
        Row::LightGlow => on(look.light_glow),
        Row::SpeedBlur => on(look.speed_blur),
        Row::Capture => format!("{}x", look.capture),
    }
}

/// The controls, as the help bar lists them.
pub const HELP_KEYS: &str = "WASD R/F Move   Arrows: Setting   Mouse/IJKL Orbit   Wheel/[ ] Zoom   Q/E Roll   Enter Photo   H Hide   C Reset   Esc Back";
pub const HELP_PAD: &str = "L-Stick Move   R-Stick Orbit   L1/R1 Zoom   D-Pad Setting   A Photo   B Back";

#[cfg(test)]
mod tests {
    use super::*;

    fn wall_x(x: f32) -> Vec<[Vec3; 3]> {
        // a wall at x, 40 m wide, 20 m tall
        let (a, b, c, d) = (Vec3::new(x, -20.0, -5.0), Vec3::new(x, 20.0, -5.0), Vec3::new(x, 20.0, 15.0), Vec3::new(x, -20.0, 15.0));
        vec![[a, b, c], [a, c, d]]
    }

    fn floor(z: f32) -> Vec<[Vec3; 3]> {
        let (a, b, c, d) = (Vec3::new(-40.0, -40.0, z), Vec3::new(40.0, -40.0, z), Vec3::new(40.0, 40.0, z), Vec3::new(-40.0, 40.0, z));
        vec![[a, b, c], [a, c, d]]
    }

    /// The eye never goes past a wall: a camera dollied out towards a wall
    /// 3 m from the car stops a sphere's radius before it, and never
    /// leaves the radius around the car or goes under the road.
    #[test]
    fn camera_is_held_by_walls_radius_and_ground() {
        let centre = Vec3::new(0.0, 0.0, 0.6);
        let mut faces = wall_x(3.0);
        faces.extend(floor(0.0));
        // looking at the car from +x: the eye wants 8 m out, the wall is at 3 m
        let rig = Rig { pan: Vec3::ZERO, yaw: std::f32::consts::FRAC_PI_2, pitch: 0.0, distance: 8.0, roll: 0.0, fov: 50.0 };
        let (pivot, eye) = rig.place(centre, &faces);
        assert_eq!(pivot, centre);
        assert!(eye.x < 3.0 - EYE_RADIUS + 0.02 && eye.x > 3.0 - EYE_RADIUS - 0.1, "{eye}");
        // no wall: the full distance
        let (_, free) = rig.place(centre, &floor(0.0));
        assert!((free.distance(centre) - 8.0).abs() < 1e-3);
        // the radius around the car: the point and the eye held
        let far = Rig { pan: Vec3::new(0.0, 30.0, 0.0), distance: MAX_DISTANCE, ..rig };
        let (p, e) = far.place(centre, &[]);
        assert!(p.distance(centre) <= PIVOT_RADIUS + 1e-3 && e.distance(centre) <= MAX_RADIUS + 1e-3);
        // under the road: an eye looking up from below stops above it
        let below = Rig { pitch: -MAX_PITCH, distance: 6.0, ..rig };
        let (_, e) = below.place(centre, &floor(0.0));
        assert!(e.z >= EYE_RADIUS - 0.02, "{e}");
        // a car against the wall: the eye still moves away from it
        let against = Vec3::new(2.85, 0.0, 0.6);
        let away = Rig { yaw: -std::f32::consts::FRAC_PI_2, distance: 5.0, ..rig };
        let (_, e) = away.place(against, &faces);
        assert!((e.distance(against) - 5.0).abs() < 1e-3, "{e}");
        // the point can't be panned through the wall either
        let through = Rig { pan: Vec3::new(6.0, 0.0, 0.0), ..rig };
        let (p, e) = through.place(centre, &faces);
        assert!(p.x < 3.0 && e.x < 3.0, "{p} {e}");
    }

    #[test]
    fn closest_point_regions() {
        let t = [Vec3::ZERO, Vec3::X, Vec3::Y];
        let near = |p: Vec3, want: Vec3| assert!((closest_on_triangle(p, &t) - want).length() < 1e-6, "{p} → {} not {want}", closest_on_triangle(p, &t));
        near(Vec3::new(0.2, 0.2, 1.0), Vec3::new(0.2, 0.2, 0.0));
        near(Vec3::new(-1.0, -1.0, 0.0), Vec3::ZERO);
        near(Vec3::new(2.0, -1.0, 0.0), Vec3::X);
        near(Vec3::new(0.5, -1.0, 3.0), Vec3::new(0.5, 0.0, 0.0));
        let e = closest_on_triangle(Vec3::new(1.0, 1.0, 0.0), &t);
        assert!((e - Vec3::new(0.5, 0.5, 0.0)).length() < 1e-6);
    }

    /// The rig taken from a camera sees the car from that camera's place.
    #[test]
    fn rig_starts_at_the_game_camera() {
        let centre = Vec3::new(10.0, 20.0, 0.6);
        let cam = Camera::looking_at(Vec3::new(10.0, 14.0, 2.0), centre);
        let rig = Rig::from_camera(&cam, centre, 4.0 / 3.0);
        let (pivot, eye) = rig.place(centre, &[]);
        assert!(eye.distance(cam.position) < 1e-3 && pivot == centre);
        assert!((rig.fov - 60.0).abs() < 0.01);
        let c = rig.camera(pivot, eye);
        assert!(c.forward().dot((centre - eye).normalize()) > 0.9999);
        // roll leans the up vector, the view axis stays
        let rolled = Rig { roll: 30.0, ..rig }.camera(pivot, eye);
        // on screen (up less its part along the view axis) the horizon leans 30°
        let f = c.forward();
        let screen = |u: Vec3| u - f * u.dot(f);
        assert!((screen(rolled.up).angle_between(screen(Vec3::Z)) - 30f32.to_radians()).abs() < 1e-3);
        assert!(rolled.forward().dot(c.forward()) > 0.9999);
    }

    /// The capture's size: the scale asked for unless the texture limit or
    /// the sample budget holds it back.
    #[test]
    fn capture_size_is_held_to_the_limits() {
        assert_eq!(capture_scale(4, (1920, 1080), 1, 16384), 4); // 7680×4320
        assert_eq!(capture_scale(4, (1920, 1080), 8, 16384), 4);
        assert_eq!(capture_scale(4, (3840, 2160), 1, 16384), 4); // 15360×8640
        assert_eq!(capture_scale(4, (3840, 2160), 8, 16384), 2); // the sample budget
        assert_eq!(capture_scale(4, (3840, 2160), 1, 8192), 2); // the texture limit
        assert_eq!(capture_scale(3, (5120, 2160), 4, 16384), 3);
        assert_eq!(capture_scale(0, (640, 480), 1, 16384), 1);
        assert_eq!(capture_scale(4, (20000, 100), 1, 16384), 1);
    }

    #[test]
    fn focus_band_widens_with_the_f_number() {
        let (n1, f1) = focus_band(10.0, 1.4);
        let (n2, f2) = focus_band(10.0, 8.0);
        assert!(n1 < 10.0 && f1 > 10.0 && n2 < n1 && f2 > f1);
        assert_eq!(focus_band(10.0, 24.0).1, f32::MAX);
        assert!(coc_scale(1.4) > coc_scale(2.8) && coc_scale(2.8) == 1.0 && coc_scale(16.0) < 0.2);
        assert_eq!(coc_scale(0.0), 0.0);
    }

    #[test]
    fn rows_step_and_hold_their_ends() {
        let mut rig = Rig { pan: Vec3::ZERO, yaw: 0.0, pitch: 0.0, distance: 5.0, roll: 0.0, fov: 50.0 };
        let mut look = Look::default();
        assert!(look.grade_is_identity());
        for _ in 0..40 {
            for r in Row::ALL {
                step(&mut rig, &mut look, r, 1);
            }
        }
        assert_eq!((rig.fov, rig.roll, look.focus, look.aperture, look.exposure, look.bleach, look.capture), (FOV_RANGE.1, MAX_ROLL, 100.0, 16.0, 3.0, 100, 4));
        for _ in 0..40 {
            for r in Row::ALL {
                step(&mut rig, &mut look, r, -1);
            }
        }
        assert_eq!((rig.fov, rig.roll, look.focus, look.aperture, look.exposure, look.bleach, look.capture), (FOV_RANGE.0, -MAX_ROLL, 0.0, 0.0, -3.0, 0, 1));
        assert_eq!(value(&rig, &look, Row::Focus), "Auto");
        assert_eq!(value(&rig, &look, Row::Aperture), "Off");
        assert!((Look { exposure: 1.0, ..look }.gain() - 2.0).abs() < 1e-6);
        assert_eq!(file_name("2026-11-04 12-00-00", 0), "Overbright 2026-11-04 12-00-00.png");
        assert_eq!(file_name("2026-11-04 12-00-00", 1), "Overbright 2026-11-04 12-00-00 (2).png");
    }

    #[test]
    fn held_controls_move_the_rig() {
        let mut rig = Rig { pan: Vec3::ZERO, yaw: 0.0, pitch: 0.2, distance: 5.0, roll: 0.0, fov: 50.0 };
        // the eye on +Y of the point: forward is −Y
        rig.drive(Vec3::new(1.0, 0.0, 0.0), (0.0, 0.0), 0.0, 0.5);
        assert!((rig.pan - Vec3::new(0.0, -2.0, 0.0)).length() < 1e-4, "{}", rig.pan);
        rig.drive(Vec3::ZERO, (0.0, 10.0), 10.0, 1.0);
        assert_eq!((rig.pitch, rig.distance), (MAX_PITCH, MIN_DISTANCE));
    }
}

#[cfg(test)]
mod between_tests {
    use super::*;

    /// The drawn rig is the older tick's at t = 0, the newest's at
    /// t = 1, halfway between at t = 0.5.
    #[test]
    fn the_drawn_rig_blends_between_ticks() {
        let a = Rig { pan: Vec3::new(0.1, 0.2, 0.3), yaw: 0.5, pitch: 0.1, distance: 4.0, roll: 0.0, fov: 40.0 };
        let b = Rig { pan: Vec3::new(0.3, 0.2, 0.1), yaw: 0.7, pitch: 0.2, distance: 5.0, roll: 2.0, fov: 44.0 };
        assert_eq!(Rig::between(&a, &b, 0.0), a);
        assert_eq!(Rig::between(&a, &b, 1.0), b);
        let m = Rig::between(&a, &b, 0.5);
        assert!((m.yaw - 0.6).abs() < 1e-6 && (m.distance - 4.5).abs() < 1e-6 && (m.fov - 42.0).abs() < 1e-5);
        assert!((m.pan - Vec3::new(0.2, 0.2, 0.2)).abs().max_element() < 1e-6);
    }
}
