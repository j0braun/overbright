use glam::{Vec2, Vec3};

use crate::render::Camera;

/// One node of the car's route for the crash camera's placement: its
/// distance (+0x54), points 2 m inside its left / right road edges and
/// the edges themselves.
#[derive(Clone, Copy, Debug)]
pub struct NodeEdges {
    pub s: f32,
    pub inner: [Vec3; 2],
    pub edge: [Vec3; 2],
}

/// The crash camera's record (director +0x268, filled by Camera_Factory
/// 0x5F1FE0 case 0x0F and re-aimed by case 0x10) as its mover 0x44BEA0
/// copies it, with the second shot's place (director +0x290).
#[derive(Clone, Debug)]
pub struct CrashShot {
    /// The eye (record +0x54 → director +0x270 / +0x280: two equal
    /// points, so the eye path 0x573490 stands still).
    pub eye: Vec3,
    /// The second shot's eye (+0x290).
    pub second: Vec3,
    /// Record +0x88: the look-at point's sideways share of the half view
    /// (−0.4 for a place on the right, 0.4 on the left; negated for the
    /// second shot).
    pub side: f32,
    /// Record +0x3C: the field of view, degrees, over 32 points (all 20,
    /// 0x8020B8; the second shot 20 − 0.25·i, 0x8020BC).
    pub fov: [f32; 32],
    /// Record +0x0C / +0x10: the progress the field of view runs over
    /// (car +0x8F8 at the start; the eye's road locator's +0x1C).
    pub from: f32,
    pub to: f32,
    /// The sight test passed: the mover runs (else the factory built a
    /// chase camera, but the record and the second place stay filled).
    pub visible: bool,
}

/// The camera spot (0x5F1FE0 case 0x0F) from the route `nodes` (the
/// car's own first), `car` (+0x60), its velocity (+0x70) and the far
/// node 0x5F1EF0 found (`far`, more than four nodes and ≥ 75 m on; none
/// refuses). The node is far − 2 when that is at least five on. Of the
/// points 2 m inside its right (vt+0x10 − 2) and left (vt+0xC + 2) edges
/// the one less far along the velocity is taken (right: side −0.4),
/// the second shot's place on the other side two nodes on from the car's;
/// unless the line from the car to it stays inside every node's edges up
/// to the one before (0x41CE90: each node's edge-to-edge line must cross
// prov: inferred event_camera.plan_shot
/// it; **inferred**: +0x18 / +0x2C as the edges), the sides swap. Both
/// eyes stand 1 m up, the first at least 1 m above the ground (0x5E72F0).
/// The sight test (0x5E7EE0) from the car to the eye then decides:
/// blocked keeps the chase camera (`visible` false).
pub fn plan_shot(
    nodes: &[NodeEdges],
    far: usize,
    car: Vec3,
    velocity: Vec3,
    ground: &dyn Fn(Vec3) -> f32,
    blocked: &dyn Fn(Vec3, Vec3) -> bool,
) -> Option<CrashShot> {
    let n = if far >= 5 { far - 2 } else { far };
    let node = nodes.get(n)?;
    let other = nodes.get(2)?;
    let along = |p: Vec3| (p - car).dot(velocity);
    let (right, left) = (node.inner[1], node.inner[0]);
    let mut pick =
        if along(right) <= along(left) { (right, -0.4f32, other.inner[0]) } else { (left, 0.4, other.inner[1]) };
    let crosses = |a: Vec2, b: Vec2, c: Vec2, d: Vec2| {
        let cross = |o: Vec2, p: Vec2, q: Vec2| (p - o).perp_dot(q - o);
        let (d1, d2) = (cross(c, d, a), cross(c, d, b));
        let (d3, d4) = (cross(a, b, c), cross(a, b, d));
        d1 * d2 <= 0.0 && d3 * d4 <= 0.0
    };
    let inside = |spot: Vec3| {
        nodes[..n].iter().all(|k| crosses(car.truncate(), spot.truncate(), k.edge[0].truncate(), k.edge[1].truncate()))
    };
    if !inside(pick.0) {
        pick = if pick.1 < 0.0 { (left, 0.4, other.inner[1]) } else { (right, -0.4, other.inner[0]) };
    }
    let (mut eye, side, mut second) = pick;
    eye.z += 1.0;
    second.z += 1.0;
    let g = ground(eye);
    if eye.z < g {
        eye.z = g + 1.0;
    }
    let visible = !blocked(car, eye);
    Some(CrashShot { eye, second, side, fov: [20.0; 32], from: 0.0, to: 1.0, visible })
}

impl CrashShot {
    /// Case 0x10: the second shot. Its eye at least at the car's height
    /// (and 1 m above the ground), the side negated, the field of view
    /// 20 − 0.25·i, the progress from the car's now; blocked from the car
    /// keeps the chase camera.
    pub fn second_shot(
        &self,
        car: Vec3,
        progress: f32,
        ground: &dyn Fn(Vec3) -> f32,
        blocked: &dyn Fn(Vec3, Vec3) -> bool,
    ) -> CrashShot {
        let mut eye = self.second;
        eye.z = eye.z.max(car.z);
        let g = ground(eye);
        if eye.z < g {
            eye.z = g + 1.0;
        }
        CrashShot {
            eye,
            second: eye,
            side: -self.side,
            fov: std::array::from_fn(|i| 20.0 - 0.25 * i as f32),
            from: progress,
            to: self.to,
            visible: !blocked(car, eye),
        }
    }

    /// The mover's update 0x44C350 for the car's centre (+0x60) and
    /// progress (+0x8F8): the field of view from the record's table at
    /// (progress − from) / (to − from) × 31 (to a binary angle, × 182.04,
    /// 0x787D8C); the look-at point the car raised by tan(half view) × 0.2
    /// (record +0x84) × the distance and moved sideways (eye − car) × up
    /// by tan(half view) × side × the distance; the eye's sway and lean
    /// (+0x40 / +0x50, the roll spline +0x3A0, the offset +0x78) are all
    /// zero from the record. The view's +0xB0 is the distance (focus);
    /// the random ±(26..44) of +0x43C only feeds +0xB4, which this mover
    /// does not write (+0x46C = 0).
    pub fn view(&self, car: Vec3, progress: f32) -> Camera {
        let f = ((progress - self.from) / (self.to - self.from)).clamp(0.0, 1.0);
        let fov = sample(&self.fov, f * 31.0);
        let bin = (fov * 182.044_45) as u16;
        let half = (bin >> 1) as f32 * std::f32::consts::TAU / 65536.0;
        let tan = half.tan();
        let offset = self.eye - car;
        let dist = offset.length().max(1.0);
        let mut look = car;
        look.z += tan * 0.2 * dist;
        let across = (offset / dist).cross(Vec3::Z) * (tan * self.side * dist);
        look.x += across.x;
        look.y += across.y;
        let mut c = Camera::looking_at(self.eye, look);
        c.game_fov = Some(bin as f32 * std::f32::consts::TAU / 65536.0);
        c.up = Vec3::Z;
        c
    }
}

/// A table read at a fractional index (the record's 32-point paths).
pub(super) fn sample(t: &[f32; 32], x: f32) -> f32 {
    let x = x.clamp(0.0, 31.0);
    let i = (x as usize).min(30);
    t[i] + (t[i + 1] - t[i]) * (x - i as f32)
}

/// 0x5F1EF0's placement on the human car's list from its node (`nodes[0]`,
/// the first at or ahead of the car): the first node at least 75 m further
/// along (+0x54) must be more than four nodes on (counting both); its
/// index (director +0x648 less +0x644), else none (no camera, no event).
pub fn far_node(nodes: &[NodeEdges]) -> Option<usize> {
    let s0 = nodes.first()?.s;
    let k = nodes.iter().position(|n| n.s - s0 >= 75.0)?;
    (k + 1 > 4).then_some(k)
}
