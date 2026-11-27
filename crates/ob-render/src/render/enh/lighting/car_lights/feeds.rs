use super::*;
// ---------------------------------------------------------------------
// The frame's cars.

static FRAME: RwLock<Option<std::sync::Arc<[Car]>>> = RwLock::new(None);

/// This frame's cars (the player's first). Empty with the tier off.
/// `on`: Headlight Beams at Next-Gen in the frame's settings.
pub fn publish(cars: Vec<Car>, on: bool) {
    *FRAME.write().unwrap() = if on { Some(cars.into()) } else { None };
}

/// The cars of the last [`publish`], shared (each of the
/// frame's several readers, one per drawn car among them, deep-copied the
/// list with every car's lights before).
pub fn cars(on: bool) -> std::sync::Arc<[Car]> {
    static EMPTY: std::sync::OnceLock<std::sync::Arc<[Car]>> = std::sync::OnceLock::new();
    let empty = || EMPTY.get_or_init(|| std::sync::Arc::from(Vec::new())).clone();
    if !on {
        return empty();
    }
    FRAME.read().unwrap().clone().unwrap_or_else(empty)
}

// ---------------------------------------------------------------------
// The world: tail / brake glows and the underglow after the beams.

/// World glows per frame at most (after the beams in the lamp list).
pub const MAX_GLOWS: usize = 24;
/// Cars whose lights reach the world at most this far from the eye.
pub(in crate::render) const WORLD_REACH: f32 = 70.0;
/// world.wgsl `beam_light`'s kinds (position.w).
pub const GLOW_POINT: f32 = 2.0;
pub const GLOW_AREA: f32 = 3.0;

/// The glows for world.wgsl's lamp list, nearest cars first. With Modern
/// Lighting's car lights in the clusters (`modern_tails`: it already has a
/// tail light per car, `modern_light_bold::car_lights`) only the braking
/// part is added.
pub fn world_glows(cars: &[Car], eye: Vec3, forward: Vec3, modern_tails: bool, look: &Look) -> Vec<GpuLamp> {
    // Every pixel of the world walks the list: cars whose light cannot
    // reach the view (behind the eye by more than the reach) are left out.
    let mut order: Vec<(f32, &Car)> = cars
        .iter()
        .map(|c| (c.position().distance(eye), c))
        .filter(|(d, c)| *d < WORLD_REACH && (c.position() - eye).dot(forward) > -(look.brake_reach + 3.0))
        .collect();
    order.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out = Vec::new();
    for (d, car) in order {
        let fade = 1.0 - ((d - 0.8 * WORLD_REACH) / (0.2 * WORLD_REACH)).clamp(0.0, 1.0);
        if let Some((p, f, c, braking)) = car.rear() {
            let c =
                if modern_tails { if braking { c - TAIL_COLOUR * (TAIL_LEVEL * 2.0) } else { Vec3::ZERO } } else { c };
            if c.max_element() > 0.0 && out.len() < MAX_GLOWS {
                let reach = if braking { look.brake_reach } else { look.tail_reach };
                let c = c.max(Vec3::ZERO) * (look.tail_road * fade);
                out.push(GpuLamp {
                    position: [p.x, p.y, p.z, GLOW_POINT],
                    colour: [c.x, c.y, c.z, reach],
                    direction: [f.x, f.y, f.z, 0.0],
                    shadow: [0.0; 4],
                });
            }
        }
        if let Some(n) = car.neon
            && out.len() < MAX_GLOWS
        {
            let (c, fwd, side, hl, hw) = car.floor();
            let n = n * (look.neon_road * fade);
            out.push(GpuLamp {
                position: [c.x, c.y, c.z + 0.12, GLOW_AREA],
                colour: [n.x, n.y, n.z, look.neon_reach],
                direction: [fwd.x, fwd.y, fwd.z, hl],
                shadow: [side.x, side.y, side.z, hw],
            });
        }
    }
    bound_glows(&mut out, look.road_budget);
    out
}

/// Scale every glow of `glows` alike so their summed
/// colour (Rec. 709 luminance) stays within `budget`: a grid of braking
/// cars lights the road like about two of them, not like all of them.
pub fn bound_glows(glows: &mut [GpuLamp], budget: f32) {
    let total: f32 = glows.iter().map(|g| g.colour[0] * 0.2126 + g.colour[1] * 0.7152 + g.colour[2] * 0.0722).sum();
    if budget > 0.0 && total > budget {
        let s = budget / total;
        for g in glows.iter_mut() {
            for c in &mut g.colour[..3] {
                *c *= s;
            }
        }
    }
}

// ---------------------------------------------------------------------
// The cars' bodies: car.wgsl's four Dynamic Lamps slots.

/// The (up to) four strongest of `lamps` (car.wgsl `Draw.lamps`, the
/// native-weight form: position + outer, colour + inner; empty slots
/// w = 0) and the car lights for a car at `at`: other cars' headlights in
/// whose cone it stands, their tail / brake lights and underglows close by,
/// and its own underglow bounced off the road onto its sills (two lights
/// beside the car at road height).
pub fn car_lamps(cars: &[Car], at: Vec3, lamps: [[f32; 4]; 8], look: &Look) -> [[f32; 4]; 8] {
    let lum = |c: Vec3| c.dot(Vec3::new(0.2126, 0.7152, 0.0722));
    // (strength, position + outer, colour + inner)
    let mut all: Vec<(f32, [f32; 4], [f32; 4])> = Vec::new();
    for i in 0..4 {
        let (p, c) = (lamps[i * 2], lamps[i * 2 + 1]);
        if p[3] <= 0.0 {
            continue;
        }
        let w = crate::render::lamps::weight(Vec3::new(p[0], p[1], p[2]).distance_squared(at), c[3], p[3]);
        all.push((w * w * lum(Vec3::new(c[0], c[1], c[2])), p, c));
    }
    let mut push = |p: Vec3, reach: f32, c: Vec3| {
        let w = crate::render::lamps::weight(p.distance_squared(at), 0.0, reach);
        if w > 0.0 && c.max_element() > 1e-4 {
            all.push((w * w * lum(c), [p.x, p.y, p.z, reach], [c.x, c.y, c.z, 0.0]));
        }
    };
    for car in cars {
        let own = car.position().distance(at) < 0.75;
        if own {
            if let Some(n) = car.neon {
                let (c, _, side, _, hw) = car.floor();
                for s in [-1.0f32, 1.0] {
                    push(c + side * (s * (hw + 0.45)) + Vec3::Z * 0.05, 1.7, n * look.glow_car);
                }
            }
            continue;
        }
        if let Some((p, f, c)) = car.heads() {
            let to = (at - p).normalize_or_zero();
            let cone = smoothstep(0.55, 0.92, f.dot(to));
            push(p, look.head_car_reach, c * (0.5 * look.head_car * cone));
        }
        if let Some((p, f, c, braking)) = car.rear() {
            let to = (at - p).normalize_or_zero();
            // Tail / brake lights shine backwards: a car beside takes little.
            let lobe = 0.05 + 0.95 * smoothstep(0.0, 0.7, f.dot(to));
            push(p, if braking { look.brake_reach } else { look.tail_reach } + 1.5, c * (look.glow_car * lobe));
        }
        if let Some(n) = car.neon {
            let (c, ..) = car.floor();
            push(c + Vec3::Z * 0.1, 3.5, n * look.glow_car);
        }
    }
    all.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut out = [[0.0; 4]; 8];
    for (i, (_, p, c)) in all.iter().take(4).enumerate() {
        out[i * 2] = *p;
        out[i * 2 + 1] = *c;
    }
    out
}

/// The brake lights as point lights in the Cinematic smoke (volume_fx
/// `SceneLights::points`: position, radius, colour); the volumetric smoke already lights
/// the smoke with every car's tail lights.
pub fn smoke_points(cars: &[Car]) -> Vec<crate::render::volume_fx::Lamp> {
    cars.iter()
        .flat_map(|c| {
            c.lights
                .iter()
                .filter(|l| l.kind == Kind::Brake)
                .map(|l| (l.position - l.facing * 0.3, 6.0, l.colour * 2.0))
        })
        .take(24)
        .collect()
}

pub(in crate::render) fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
