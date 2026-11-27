use glam::{Vec2, Vec3};

// ---------------------------------------------------------------------
// Puddles on the CPU (the spray's splashes): world.wgsl's field.

fn hash12(p: Vec2) -> f32 {
    let fract = |v: f32| v - v.floor();
    let mut p3 = Vec3::new(fract(p.x * 0.1031), fract(p.y * 0.1031), fract(p.x * 0.1031));
    let d = p3.dot(Vec3::new(p3.y, p3.z, p3.x) + 33.33);
    p3 += d;
    fract((p3.x + p3.y) * p3.z)
}

fn noise(p: Vec2) -> f32 {
    let i = p.floor();
    let f = p - i;
    let u = f * f * (Vec2::splat(3.0) - 2.0 * f);
    let a = hash12(i);
    let b = hash12(i + Vec2::X);
    let c = hash12(i + Vec2::Y);
    let d = hash12(i + Vec2::ONE);
    let k = a - b - c + d;
    a + (b - a) * u.x + (c - a) * u.y + k * u.x * u.y
}

pub(crate) fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The puddle at `p` on a fully reflective road of wetness level `level`
/// (wetness.wgsl `wetness_puddle`, which world.wgsl `wet_surface` and Rain
/// Next-Gen's rings read: the field and threshold, the puddles drying more
/// slowly than the film, √level).
pub fn puddle_at(p: Vec2, level: f32) -> f32 {
    let field = 0.65 * noise(p * (1.0 / 7.0)) + 0.35 * noise(p * (1.0 / 2.3) + Vec2::splat(17.0));
    let edge = 0.66 - 0.22 * level;
    smoothstep(edge, edge + 0.08, field) * level.max(0.0).sqrt()
}

/// The road's wetness level the shader uses (wet_surface's `level`): the one wetness's film
/// ([`super::wetness::Wetness::film`]).
pub fn level(rain: f32, road_wet: f32) -> f32 {
    super::wetness::Wetness::of(rain, road_wet).film
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The CPU puddles follow world.wgsl's field: none on a dry road, some
    /// of the road under water when wet, more the wetter.
    #[test]
    fn puddles_grow_with_the_wetness() {
        let share = |lv: f32| {
            (0..4000).filter(|i| puddle_at(Vec2::new((*i % 63) as f32 * 1.7, (*i / 63) as f32 * 1.3), lv) > 0.2).count()
                as f32
                / 4000.0
        };
        assert_eq!(share(0.0), 0.0);
        let (a, b) = (share(0.4), share(1.0));
        assert!(b > a && b > 0.03 && b < 0.8, "{a} {b}");
        assert_eq!(level(0.0, 0.0), 0.0);
        assert!((level(1.0, 1.0) - 1.0).abs() < 1e-6);
    }
}
