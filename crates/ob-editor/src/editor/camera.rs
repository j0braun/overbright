//! The viewport's fly camera and the maths that maps between the world and
//! the viewport's rectangle: projecting a world point to a screen point,
//! the mouse ray through a screen point, framing a sphere.
//!
//! The camera is the engine's [`Camera`] (Z up, yaw 0 looks down +Y,
//! positive pitch looks up), so the world renderer draws exactly what these
//! functions compute. Everything here is windowless and GPU-free.

use egui::{Pos2, Rect};
use glam::{Vec2, Vec3, Vec4};

use crate::draw::camera::Camera;

/// The slowest and fastest fly speeds (m/s); the mouse wheel steps between
/// them by [`SPEED_STEP`].
pub const MIN_SPEED: f32 = 0.5;
pub const MAX_SPEED: f32 = 200.0;
pub const DEFAULT_SPEED: f32 = 20.0;
/// One wheel notch multiplies or divides the speed by this.
pub const SPEED_STEP: f32 = 1.25;
/// Mouse look, radians per logical pixel of drag.
pub const LOOK_PER_PIXEL: f32 = 0.004;
/// Pitch stops short of straight up / down (the view matrix needs a
/// forward that is not parallel to +Z).
pub const MAX_PITCH: f32 = 1.55;
/// Shift / Ctrl while flying.
pub const FAST: f32 = 4.0;
pub const SLOW: f32 = 0.25;

/// One frame's fly input, read from the keyboard and mouse while the right
/// button is held over the viewport.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FlyInput {
    /// W − S: along the view direction (pitch included).
    pub forward: f32,
    /// D − A: sideways, level.
    pub right: f32,
    /// E − Q: world up / down.
    pub up: f32,
    /// Mouse movement since the last frame, logical pixels (x right, y down).
    pub look: Vec2,
    pub fast: bool,
    pub slow: bool,
}

impl FlyInput {
    /// Whether this input moves or turns the camera at all.
    pub fn is_idle(&self) -> bool {
        self.forward == 0.0 && self.right == 0.0 && self.up == 0.0 && self.look == Vec2::ZERO
    }
}

/// The fly camera's own state beside the [`Camera`] in the editor state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlyCam {
    /// Metres per second at normal speed.
    pub speed: f32,
}

impl Default for FlyCam {
    fn default() -> FlyCam {
        FlyCam { speed: DEFAULT_SPEED }
    }
}

impl FlyCam {
    /// One wheel movement of `notches` (positive = faster).
    pub fn wheel(&mut self, notches: f32) {
        self.speed = (self.speed * SPEED_STEP.powf(notches)).clamp(MIN_SPEED, MAX_SPEED);
    }

    /// Move and turn `cam` by `input` over `dt` seconds. Movement scales
    /// with `dt`, so the result does not depend on the frame rate; the look
    /// is the mouse's own movement and needs no `dt`.
    pub fn apply(&self, cam: &mut Camera, input: &FlyInput, dt: f32) {
        cam.yaw += input.look.x * LOOK_PER_PIXEL;
        cam.pitch = (cam.pitch - input.look.y * LOOK_PER_PIXEL).clamp(-MAX_PITCH, MAX_PITCH);
        let mut speed = self.speed;
        if input.fast {
            speed *= FAST;
        }
        if input.slow {
            speed *= SLOW;
        }
        let forward = cam.forward();
        let right = level_right(cam);
        let v = forward * input.forward + right * input.right + Vec3::Z * input.up;
        cam.position += v * speed * dt;
    }
}

/// The camera's right, level with the ground.
fn level_right(cam: &Camera) -> Vec3 {
    let (s, c) = cam.yaw.sin_cos();
    Vec3::new(c, -s, 0.0)
}

/// The viewport's aspect ratio (1 for an empty rectangle).
pub fn aspect(rect: Rect) -> f32 {
    if rect.width() > 0.0 && rect.height() > 0.0 { rect.width() / rect.height() } else { 1.0 }
}

/// Where world point `p` lands in `rect`, or `None` when it is behind the
/// camera's near plane. The point may lie outside `rect` (callers clip).
pub fn project(cam: &Camera, rect: Rect, p: Vec3) -> Option<Pos2> {
    let clip = cam.view_projection(aspect(rect)) * p.extend(1.0);
    clip_to_screen(clip, rect, cam.near)
}

/// A clip-space point to the rectangle, `None` in front of `near`.
fn clip_to_screen(clip: Vec4, rect: Rect, near: f32) -> Option<Pos2> {
    // For a right-handed perspective clip.w is the distance along the view
    // direction.
    if clip.w < near {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    Some(ndc_to_screen(Vec2::new(ndc.x, ndc.y), rect))
}

fn ndc_to_screen(ndc: Vec2, rect: Rect) -> Pos2 {
    Pos2::new(rect.min.x + (ndc.x + 1.0) * 0.5 * rect.width(), rect.min.y + (1.0 - ndc.y) * 0.5 * rect.height())
}

/// The mouse ray through screen point `at` in `rect`: the camera's eye and a
/// unit direction.
pub fn ray(cam: &Camera, rect: Rect, at: Pos2) -> (Vec3, Vec3) {
    let x = (at.x - rect.min.x) / rect.width().max(1.0) * 2.0 - 1.0;
    let y = 1.0 - (at.y - rect.min.y) / rect.height().max(1.0) * 2.0;
    let inv = cam.view_projection(aspect(rect)).inverse();
    let far = inv * Vec4::new(x, y, 1.0, 1.0);
    let near = inv * Vec4::new(x, y, 0.0, 1.0);
    let (far, near) = (far.truncate() / far.w, near.truncate() / near.w);
    let dir = (far - near).normalize_or(cam.forward());
    (cam.position, dir)
}

/// The camera pulled back along its own view direction until a sphere of
/// `radius` around `centre` fills about two thirds of the shorter side.
pub fn frame(cam: &mut Camera, centre: Vec3, radius: f32, aspect: f32) {
    // The narrower half-angle: vertical, or horizontal on a tall viewport.
    let half_y = cam.fov_y * 0.5;
    let half_x = (half_y.tan() * aspect.max(1e-3)).atan();
    let half = half_y.min(half_x);
    let distance = (radius.max(0.5) / half.sin() * 1.5).max(cam.near * 4.0);
    cam.position = centre - cam.forward() * distance;
}

/// The segment `a`–`b` cut to the part in front of the camera's near plane
/// (`None` if none of it is), so a line that passes behind the camera still
/// draws the part the viewer can see.
pub fn clip_segment(cam: &Camera, a: Vec3, b: Vec3) -> Option<(Vec3, Vec3)> {
    let f = cam.forward();
    let near = cam.near * 1.01;
    let da = (a - cam.position).dot(f) - near;
    let db = (b - cam.position).dot(f) - near;
    match (da >= 0.0, db >= 0.0) {
        (true, true) => Some((a, b)),
        (false, false) => None,
        (true, false) => Some((a, a + (b - a) * (da / (da - db)))),
        (false, true) => Some((a + (b - a) * (da / (da - db)), b)),
    }
}

/// Both ends of a world segment on screen, cut at the near plane.
pub fn project_segment(cam: &Camera, rect: Rect, a: Vec3, b: Vec3) -> Option<(Pos2, Pos2)> {
    let (a, b) = clip_segment(cam, a, b)?;
    Some((project(cam, rect, a)?, project(cam, rect, b)?))
}

// --- the top-down overview -----------------------------------------------------

/// The overview's field of view: so narrow that the camera, far above the
/// region, sees it as an orthographic view would (a 50 m building at the
/// edge of the image leans by under a metre), while the world renderer, its
/// culling and the editor's picking keep working with an ordinary
/// perspective camera.
pub const TOP_FOV: f32 = 2.0 * std::f32::consts::PI / 180.0;
/// The overview's depth range around the ground: the highest and lowest
/// world geometry it draws (hills, bridges, the harbour floor).
pub const TOP_CEILING: f32 = 2500.0;
pub const TOP_FLOOR: f32 = -1500.0;
/// The overview's zoom limits: metres from the image's middle to its top
/// edge.
pub const TOP_MIN_HALF: f32 = 20.0;
pub const TOP_MAX_HALF: f32 = 30_000.0;

/// The top-down overview's pan and zoom: north (+Y) up, east (+X) right.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TopView {
    /// The world point (x, y) at the image's middle.
    pub centre: Vec2,
    /// Metres from the middle to the top edge.
    pub half_height: f32,
}

impl Default for TopView {
    fn default() -> TopView {
        TopView { centre: Vec2::ZERO, half_height: 2000.0 }
    }
}

impl TopView {
    /// The camera that draws this view: straight down from far above,
    /// north up, its near and far planes around [`TOP_CEILING`] and
    /// [`TOP_FLOOR`].
    pub fn camera(&self) -> Camera {
        // `half_height` is measured on the ground (z = 0).
        let distance = self.half_height / (TOP_FOV * 0.5).tan();
        // A hair off straight down, so the camera's level right is
        // defined; the eye moved back by as much, so the view's middle
        // stays on `centre`.
        let mut c = Camera::new(Vec3::ZERO, 0.0, -std::f32::consts::FRAC_PI_2 + 1e-5);
        let f = c.forward();
        c.position = self.centre.extend(0.0) - f * (distance / -f.z);
        c.up = Vec3::Y;
        c.fov_y = TOP_FOV;
        c.near = (distance - TOP_CEILING).max(1.0);
        c.far = distance - TOP_FLOOR;
        c
    }

    /// Metres per screen point in `rect`.
    pub fn metres_per_point(&self, rect: Rect) -> f32 {
        2.0 * self.half_height / rect.height().max(1.0)
    }

    /// A screen point in `rect` on the ground plane (x, y).
    pub fn to_world(&self, rect: Rect, at: Pos2) -> Vec2 {
        let m = self.metres_per_point(rect);
        let c = rect.center();
        Vec2::new(self.centre.x + (at.x - c.x) * m, self.centre.y - (at.y - c.y) * m)
    }

    /// Pan by a screen movement (the ground follows the mouse).
    pub fn pan(&mut self, rect: Rect, by: egui::Vec2) {
        let m = self.metres_per_point(rect);
        self.centre += Vec2::new(-by.x, by.y) * m;
    }

    /// Zoom by `factor` (> 1 closer) keeping the ground under `at` still.
    pub fn zoom_at(&mut self, rect: Rect, at: Pos2, factor: f32) {
        let before = self.to_world(rect, at);
        self.half_height = (self.half_height / factor.max(1e-3)).clamp(TOP_MIN_HALF, TOP_MAX_HALF);
        let after = self.to_world(rect, at);
        self.centre += before - after;
    }

    /// Show the box `lo`..`hi` (world x, y) whole in `rect`, with a margin.
    pub fn fit(&mut self, rect: Rect, lo: Vec2, hi: Vec2) {
        let size = (hi - lo).max(Vec2::splat(1.0));
        self.centre = (lo + hi) * 0.5;
        let half = (size.y * 0.5).max(size.x * 0.5 / aspect(rect));
        self.half_height = (half * 1.06).clamp(TOP_MIN_HALF, TOP_MAX_HALF);
    }
}

/// The perspective camera a double-click in the overview flies to: above
/// `target`, looking at it from the south-ish side the way `from` looked,
/// high enough to see the blocks around it.
pub fn fly_to(from: &Camera, target: Vec3) -> Camera {
    let f = from.forward();
    let heading = Vec2::new(f.x, f.y).normalize_or(Vec2::Y);
    let eye = target - heading.extend(0.0) * 90.0 + Vec3::Z * 70.0;
    let mut c = Camera::looking_at(eye, target);
    c.fov_y = from.fov_y.clamp(0.3, 2.0);
    c
}

/// The whole-map view's camera: `cam` with its near and far planes set so
/// a region `radius` metres across from the camera is inside them, and the
/// near plane pushed out with the height above the ground (`height`) so
/// the depth buffer keeps its precision.
pub fn whole_camera(cam: &Camera, height: f32, radius: f32) -> Camera {
    let mut c = *cam;
    c.far = c.far.max(radius * 1.2 + height.max(0.0) * 2.0);
    c.near = (height * 0.02).clamp(c.near, 25.0);
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect() -> Rect {
        Rect::from_min_size(Pos2::new(100.0, 50.0), egui::vec2(800.0, 600.0))
    }

    #[test]
    fn a_point_straight_ahead_lands_in_the_middle() {
        let cam = Camera::new(Vec3::new(10.0, 20.0, 5.0), 0.7, -0.2);
        let p = cam.position + cam.forward() * 50.0;
        let s = project(&cam, rect(), p).unwrap();
        assert!((s - rect().center()).length() < 0.01, "{s:?}");
    }

    #[test]
    fn a_point_behind_the_camera_does_not_project() {
        let cam = Camera::new(Vec3::ZERO, 0.0, 0.0);
        assert!(project(&cam, rect(), Vec3::new(0.0, -10.0, 0.0)).is_none());
    }

    #[test]
    fn up_and_right_land_up_and_right() {
        // Yaw 0 looks down +Y; right is +X.
        let cam = Camera::new(Vec3::ZERO, 0.0, 0.0);
        let c = rect().center();
        let r = project(&cam, rect(), Vec3::new(5.0, 50.0, 0.0)).unwrap();
        let u = project(&cam, rect(), Vec3::new(0.0, 50.0, 5.0)).unwrap();
        assert!(r.x > c.x && (r.y - c.y).abs() < 0.01);
        assert!(u.y < c.y && (u.x - c.x).abs() < 0.01);
    }

    #[test]
    fn the_ray_through_a_projected_point_passes_through_it() {
        let cam = Camera::new(Vec3::new(-30.0, 4.0, 12.0), -1.1, -0.35);
        for p in [Vec3::new(-10.0, 10.0, 0.0), Vec3::new(-25.0, -6.0, 3.0), Vec3::new(-40.0, 30.0, -8.0)] {
            let Some(s) = project(&cam, rect(), p) else { continue };
            let (eye, dir) = ray(&cam, rect(), s);
            assert!((dir.length() - 1.0).abs() < 1e-4);
            let t = (p - eye).dot(dir);
            let miss = (eye + dir * t - p).length();
            assert!(miss < 0.01 * t.max(1.0), "{p:?} missed by {miss}");
        }
    }

    #[test]
    fn the_ray_through_the_centre_is_the_view_direction() {
        let cam = Camera::new(Vec3::ZERO, 2.0, 0.4);
        let (_, dir) = ray(&cam, rect(), rect().center());
        assert!(dir.distance(cam.forward()) < 1e-4);
    }

    #[test]
    fn flying_is_frame_rate_independent() {
        let fly = FlyCam::default();
        let input = FlyInput { forward: 1.0, right: 0.5, up: -1.0, ..Default::default() };
        let mut one = Camera::new(Vec3::ZERO, 0.3, 0.0);
        let mut many = one;
        fly.apply(&mut one, &input, 1.0);
        for _ in 0..200 {
            fly.apply(&mut many, &input, 1.0 / 200.0);
        }
        assert!(one.position.distance(many.position) < 1e-3);
        assert!((one.position.length() - (1.0f32 + 0.25 + 1.0).sqrt() * DEFAULT_SPEED).abs() < 0.1);
    }

    #[test]
    fn fast_slow_and_the_wheel() {
        let mut fly = FlyCam::default();
        let fwd = FlyInput { forward: 1.0, ..Default::default() };
        let run = |fly: &FlyCam, input: FlyInput| {
            let mut c = Camera::new(Vec3::ZERO, 0.0, 0.0);
            fly.apply(&mut c, &input, 1.0);
            c.position.length()
        };
        assert!((run(&fly, FlyInput { fast: true, ..fwd }) - DEFAULT_SPEED * FAST).abs() < 1e-3);
        assert!((run(&fly, FlyInput { slow: true, ..fwd }) - DEFAULT_SPEED * SLOW).abs() < 1e-3);
        fly.wheel(1.0);
        assert!((fly.speed - DEFAULT_SPEED * SPEED_STEP).abs() < 1e-3);
        fly.wheel(-100.0);
        assert_eq!(fly.speed, MIN_SPEED);
        fly.wheel(100.0);
        assert_eq!(fly.speed, MAX_SPEED);
    }

    #[test]
    fn looking_turns_and_stops_at_the_poles() {
        let fly = FlyCam::default();
        let mut c = Camera::new(Vec3::ZERO, 0.0, 0.0);
        fly.apply(&mut c, &FlyInput { look: Vec2::new(100.0, -10_000.0), ..Default::default() }, 0.016);
        assert!((c.yaw - 0.4).abs() < 1e-5);
        assert_eq!(c.pitch, MAX_PITCH);
        assert_eq!(c.position, Vec3::ZERO);
    }

    #[test]
    fn framing_puts_the_sphere_in_view() {
        let mut cam = Camera::new(Vec3::new(500.0, 500.0, 500.0), 1.0, -0.3);
        let centre = Vec3::new(3.0, 4.0, 1.0);
        for aspect in [16.0 / 9.0, 0.5] {
            frame(&mut cam, centre, 5.0, aspect);
            let r = Rect::from_min_size(Pos2::ZERO, egui::vec2(600.0 * aspect, 600.0));
            let s = project(&cam, r, centre).unwrap();
            assert!((s - r.center()).length() < 0.01);
            // Points on the sphere's edge across the view stay on screen.
            let side = level_right(&cam) * 5.0;
            for p in [centre + side, centre - side, centre + Vec3::Z * 5.0] {
                assert!(r.contains(project(&cam, r, p).unwrap()), "{aspect}: {p:?}");
            }
        }
    }

    #[test]
    fn segments_crossing_the_near_plane_are_cut() {
        let cam = Camera::new(Vec3::ZERO, 0.0, 0.0);
        let (a, b) = (Vec3::new(1.0, 10.0, 0.0), Vec3::new(1.0, -10.0, 0.0));
        let (ca, cb) = clip_segment(&cam, a, b).unwrap();
        assert_eq!(ca, a);
        assert!(cb.y > 0.0 && cb.y < 1.0);
        assert!(project_segment(&cam, rect(), a, b).is_some());
        assert!(clip_segment(&cam, -a, -a * 2.0).is_none());
    }

    #[test]
    fn the_overview_looks_straight_down_north_up() {
        let r = rect();
        let v = TopView { centre: Vec2::new(500.0, -300.0), half_height: 1000.0 };
        let cam = v.camera();
        assert!(cam.forward().z < -0.999);
        // The middle of the view, east and north.
        let c = project(&cam, r, Vec3::new(500.0, -300.0, 0.0)).unwrap();
        assert!((c - r.center()).length() < 0.05, "{c:?}");
        let e = project(&cam, r, Vec3::new(600.0, -300.0, 0.0)).unwrap();
        let n = project(&cam, r, Vec3::new(500.0, -200.0, 0.0)).unwrap();
        assert!(e.x > c.x && (e.y - c.y).abs() < 0.05);
        assert!(n.y < c.y && (n.x - c.x).abs() < 0.05);
        // The top edge is half_height north of the middle.
        let top = project(&cam, r, Vec3::new(500.0, 700.0, 0.0)).unwrap();
        assert!((top.y - r.top()).abs() < 0.5, "{top:?}");
        // Nearly orthographic: a 50 m tall point at the corner leans < 1 px.
        let corner = Vec3::new(500.0 + 1300.0, 700.0, 0.0);
        let lean = project(&cam, r, corner + Vec3::Z * 50.0).unwrap() - project(&cam, r, corner).unwrap();
        assert!(lean.length() < 1.0, "{lean:?}");
        // The whole height range is inside the depth range.
        for z in [TOP_FLOOR + 1.0, 0.0, TOP_CEILING - 1.0] {
            let clip = cam.view_projection(aspect(r)) * Vec3::new(500.0, -300.0, z).extend(1.0);
            let depth = clip.z / clip.w;
            assert!((0.0..=1.0).contains(&depth), "{z}: {depth}");
        }
    }

    #[test]
    fn overview_maths_matches_the_projection() {
        let r = rect();
        let v = TopView { centre: Vec2::new(-1200.0, 800.0), half_height: 3000.0 };
        let cam = v.camera();
        for at in [Pos2::new(130.0, 70.0), r.center(), Pos2::new(880.0, 600.0)] {
            let w = v.to_world(r, at);
            let back = project(&cam, r, w.extend(0.0)).unwrap();
            assert!((back - at).length() < 0.5, "{at:?} -> {w:?} -> {back:?}");
            // The mouse ray meets the ground where to_world says.
            let (eye, dir) = ray(&cam, r, at);
            let hit = eye + dir * (-eye.z / dir.z);
            assert!(hit.truncate().distance(w) < 2.0, "{hit:?} vs {w:?}");
        }
    }

    #[test]
    fn overview_pan_zoom_and_fit() {
        let r = rect();
        let mut v = TopView::default();
        let p = Vec2::new(250.0, 40.0);
        let at = Pos2::new(300.0, 200.0);
        v.centre = p - (v.to_world(r, at) - v.centre);
        let before = v.to_world(r, at);
        v.zoom_at(r, at, 3.0);
        assert!(v.to_world(r, at).distance(before) < 1e-2);
        let w = v.to_world(r, at);
        v.pan(r, egui::vec2(40.0, -25.0));
        assert!(v.to_world(r, at + egui::vec2(40.0, -25.0)).distance(w) < 1e-2);
        let (lo, hi) = (Vec2::new(-9000.0, -4000.0), Vec2::new(5000.0, 6000.0));
        v.fit(r, lo, hi);
        let cam = v.camera();
        for p in [lo, hi, Vec2::new(lo.x, hi.y), Vec2::new(hi.x, lo.y)] {
            assert!(r.contains(project(&cam, r, p.extend(0.0)).unwrap()), "{p:?}");
        }
        v.zoom_at(r, at, 1e-9);
        assert_eq!(v.half_height, TOP_MAX_HALF);
    }

    #[test]
    fn flying_to_a_point_looks_at_it() {
        let from = Camera::new(Vec3::new(0.0, 0.0, 5000.0), 0.4, -1.2);
        let target = Vec3::new(300.0, -200.0, 12.0);
        let c = fly_to(&from, target);
        let s = project(&c, rect(), target).unwrap();
        assert!((s - rect().center()).length() < 0.5);
        assert!(c.position.z > target.z + 50.0);
    }

    #[test]
    fn the_whole_map_camera_reaches_across_the_region() {
        let cam = Camera::new(Vec3::new(0.0, 0.0, 3000.0), 0.0, -0.9);
        let c = whole_camera(&cam, 3000.0, 20_000.0);
        assert!(c.far >= 24_000.0 && c.near > 0.5 && c.near <= 25.0);
        let low = whole_camera(&cam, 2.0, 20_000.0);
        assert_eq!(low.near, cam.near);
    }
}
