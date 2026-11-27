use super::*;


/// Props fade (shrink) out between these distances (m).
pub const FADE: (f32, f32) = (42.0, 60.0);
/// Loose debris is simulated within this distance of the eye (m).
pub const SIM_RADIUS: f32 = 40.0;
/// Most loose props moved at once (the farthest settled ones are reset).
pub const MAX_LOOSE: usize = 3000;
/// Grit chips at once.
pub const MAX_GRIT: usize = 600;
/// Instances drawn a frame at most.
pub const MAX_INSTANCES: usize = 32768;
/// Steam vents fed at once (the nearest).
pub const MAX_VENTS: usize = 10;
/// The volume's grid: 64 × 64 × 16 m of 0.5 m voxels.
pub const BUDGET: volume_fx::Budget =
    volume_fx::Budget { dims: [128, 128, 32], voxel: 0.5, particles: 8192, steps: 56 };

/// A car at a tick, as the game's tick hands it.
#[derive(Clone, Copy, Debug)]
pub struct CarIn {
    pub world: Mat4,
    pub velocity: Vec3,
    /// Per wheel: the ground contact (world), slip, on the ground.
    pub wheels: [(Vec3, f32, bool); 4],
    pub headlight: [u8; 3],
    pub braking: bool,
}

/// A car at a tick (built by the game's tick from the car's
/// state; the draw's glow pass pushed it before): the car's matrix and
/// velocity, its wheels (car-space centre and radius) and their slip /
/// ground state, the headlight colour and the brake flag.
pub fn car_in(
    world: Mat4,
    velocity: Vec3,
    wheels: [(Vec3, f32); 4],
    state: [(f32, bool); 4],
    headlight: [u8; 3],
    braking: bool,
) -> CarIn {
    let contacts: [(Vec3, f32, bool); 4] = std::array::from_fn(|i| {
        let (c, r) = wheels[i];
        (world.transform_point3(c - Vec3::Z * r), state[i].0, state[i].1)
    });
    CarIn { world, velocity, wheels: contacts, headlight, braking }
}

/// A tick's cars (`cars`): none with the row off, 32 at most, the same
/// place twice once (`tier`: the row as the tick took it).
pub fn add_car(cars: &mut Vec<CarIn>, car: CarIn, tier: u8) {
    if tier == 0 {
        return;
    }
    let at = car.world.w_axis.truncate();
    if cars.len() >= 32 || cars.iter().any(|c| c.world.w_axis.truncate().distance_squared(at) < 1e-4) {
        return;
    }
    cars.push(car);
}

/// The air (m/s) a car at `car` moving at `v` pushes at `p`: the bow and
/// side wash (outward along its length) and the trailing wake (dragged
/// along, rising), fading with height. **Inferred**, by eye.
pub fn car_air(p: Vec3, car: Vec3, v: Vec3) -> Vec3 {
    let vh = Vec3::new(v.x, v.y, 0.0);
    let s = vh.length();
    if s < 3.0 {
        return Vec3::ZERO;
    }
    let dir = vh / s;
    let r = p - car;
    let along = r.dot(dir);
    let lat = Vec3::new(r.x, r.y, 0.0) - dir * along;
    let latd = lat.length();
    let height = (-(r.z.max(0.0)) / 1.5).exp();
    let out = if latd > 1e-3 { lat / latd } else { Vec3::ZERO };
    // Side wash along the body (-2.5 .. 3 m), from its side outward.
    let win = smooth(-3.5, -2.0, along) * (1.0 - smooth(2.0, 3.5, along));
    let side = out * s * 0.18 * (-((latd - 1.0).max(0.0) / 2.0).powi(2)).exp() * win;
    // The wake behind it.
    let behind = (-along - 2.0).max(0.0);
    let reach = 4.0 + 0.25 * s;
    let spread = 1.2 + 0.18 * behind;
    let wake = smooth(-1.0, -2.5, along) * (-behind / reach).exp() * (-(latd / spread).powi(2)).exp();
    let drag = dir * s * 0.45 * wake + Vec3::Z * s * 0.12 * wake + out * s * 0.05 * wake;
    (side + drag) * height
}

pub(super) fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// 1D value noise in 0..1 (own code).
pub(super) fn noise1(x: f32, salt: u64) -> f32 {
    let i = x.floor();
    let f = x - i;
    let h = |k: f32| (props::hash(salt ^ (k as i64 as u64)) >> 40) as f32 / (1u64 << 24) as f32;
    let w = f * f * (3.0 - 2.0 * f);
    h(i) + (h(i + 1.0) - h(i)) * w
}

/// The base wind (volume_fx's, **inferred**: NFSU2 has none) plus slow
/// gusts that sweep the streets now and then (up to ~4.5 m/s).
pub fn wind(p: Vec3, time: f32) -> Vec3 {
    let base = Fields::default().wind;
    let n = noise1(time * 0.11 + p.x * 0.006 + p.y * 0.004, 0x6755_5354) * 0.7
        + noise1(time * 0.37 + p.y * 0.01, 0x4755_5354) * 0.3;
    let g = smooth(0.68, 0.95, n) * 3.4;
    let turn = noise1(time * 0.05, 0x5455_524E) * 1.2 - 0.6;
    let d = Quat::from_rotation_z(turn) * base.normalize_or(Vec3::X);
    base + d * g
}

/// A loose prop moved off its place (Next-Gen).
#[derive(Clone, Copy, Debug)]
pub(super) struct Loose {
    pub(super) pos: Vec3,
    pub(super) vel: Vec3,
    pub(super) rot: Quat,
    pub(super) spin: Vec3,
    pub(super) flying: bool,
    pub(super) calm: f32,
}

/// A grit chip.
#[derive(Clone, Copy, Debug)]
pub(super) struct Grit {
    pub(super) pos: Vec3,
    pub(super) vel: Vec3,
    pub(super) rot: Quat,
    pub(super) age: f32,
    pub(super) seed: f32,
    pub(super) light: Vec3,
}

/// A lift threshold (m/s of air) per loose kind and seed.
pub(super) fn threshold(kind: Kind, seed: f32) -> f32 {
    (if kind == Kind::Leaf { 2.3 } else { 3.0 }) * (0.85 + 0.6 * seed)
}

fn flat_rot(n: Vec3, rot: Quat) -> Quat {
    let x = rot * Vec3::X;
    let yaw = x.y.atan2(x.x);
    Quat::from_rotation_arc(Vec3::Z, n) * Quat::from_rotation_z(yaw)
}

/// One step of a loose card in the air or on the ground.
pub(super) fn step_loose(
    l: &mut Loose,
    kind: Kind,
    seed: f32,
    air: Vec3,
    time: f32,
    h: f32,
    ground: Option<(f32, Vec3, bool)>,
    rest: (f32, Vec3),
) -> bool {
    let thr = threshold(kind, seed);
    let rel = air - l.vel;
    let relh = Vec3::new(rel.x, rel.y, 0.0);
    let (gz, gn) = ground.map_or(rest, |(z, n, _)| (z, n));
    if !l.flying {
        if Vec3::new(air.x, air.y, 0.0).length() > thr {
            l.flying = true;
            l.calm = 0.0;
            l.vel += Vec3::Z * (0.6 + 0.8 * seed);
        } else {
            return false;
        }
    }
    let k = if kind == Kind::Leaf { 2.6 } else { 2.0 };
    let mut acc = Vec3::new(rel.x * k, rel.y * k, rel.z * 5.0) - Vec3::Z * 9.81;
    acc += Vec3::Z * (0.07 * relh.length_squared()).min(14.0);
    // Flutter: sideways and up-down swings as it falls.
    let side = Vec3::new(-relh.y, relh.x, 0.0).normalize_or_zero();
    acc += side * (time * 7.0 + seed * 40.0).sin() * (0.6 + 0.25 * relh.length())
        + Vec3::Z * (time * 5.3 + seed * 13.0).sin() * 1.8;
    l.vel += acc * h;
    l.pos += l.vel * h;
    let axis = Vec3::new((seed * 91.0).sin(), (seed * 57.0).cos(), 0.6).normalize();
    l.spin += (axis * rel.length() * 3.0 - l.spin * 2.0) * h;
    let a = l.spin.length() * h;
    if a > 1e-6 {
        l.rot = (Quat::from_axis_angle(l.spin / l.spin.length(), a) * l.rot).normalize();
    }
    let rest_z = gz + 0.004;
    if l.pos.z <= rest_z {
        l.pos.z = rest_z;
        l.vel.z = l.vel.z.max(0.0);
        let f = (-6.0 * h).exp();
        l.vel.x *= f;
        l.vel.y *= f;
        let flat = flat_rot(gn, l.rot);
        l.rot = l.rot.slerp(flat, 1.0 - (-10.0 * h).exp());
        l.spin *= (-10.0 * h).exp();
        if l.vel.length() < 0.15 && relh.length() < thr {
            l.calm += h;
            if l.calm > 0.35 {
                l.flying = false;
                l.vel = Vec3::ZERO;
                l.spin = Vec3::ZERO;
                l.rot = flat;
            }
        }
    } else {
        l.calm = 0.0;
    }
    true
}

/// What one tick hands Ambient Detail ([`AmbientDetail::tick`]):
/// the cars (the glow pass's list), the tick's eye, the draw's clock as
/// the next frame reads it, the road's wetness.
pub struct AmbientTick {
    /// The row's tier as the tick took it (0 Off, 1 On, 2 Next-Gen).
    pub tier: u8,
    pub cars: Vec<CarIn>,
    pub eye: Vec3,
    pub time: f32,
    pub wet: f32,
}
