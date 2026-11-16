//! Phase 4 (rate independence, `audit/arch/phase4-rate.md` §1.3/§1.4):
//! the native per-frame visuals in their tick form. Each effect keeps a
//! state that a tick steps exactly as the original steps it once per 60 fps
//! frame, and a pure function that turns the snapshots of two ticks
//! (S_{n−1}, S_n) and α ∈ [0, 1) into what a drawn frame uses. At α = 0 the
//! drawn value is S_{n−1} bit for bit; between ticks it lies within the two.
//!
//! The per-frame entry points the game calls today run over the same code:
//! one step, then the draw at α = 0 ([`crate::render::native::fx`]'s
//! `fog`, `effect_controller` (screen tint), `horizon_fog`, `motion_blur`,
//! `glow::Pools`; `post::OverbrightLevel`; `display::env_faces_tick`).

/// A drawn frame's value of a continuous scalar between tick `n − 1` (`a`)
/// and tick `n` (`b`): exactly `a` at α = 0 (and for α ≤ 0 or NaN), never
/// outside [a, b] (clamped against the f32 rounding of the lerp), `b` from
/// α ≥ 1.
#[inline]
pub fn lerp(a: f32, b: f32, alpha: f32) -> f32 {
    // `!(alpha > 0)` also catches NaN.
    if !(alpha > 0.0) || a == b {
        return a;
    }
    if alpha >= 1.0 {
        return b;
    }
    let v = a + (b - a) * alpha;
    if a <= b { v.clamp(a, b) } else { v.clamp(b, a) }
}

/// [`lerp`] per element.
#[inline]
pub fn lerp_n<const N: usize>(a: [f32; N], b: [f32; N], alpha: f32) -> [f32; N] {
    std::array::from_fn(|k| lerp(a[k], b[k], alpha))
}

/// Whether `v` lies within the two tick values (the plan's bound for the
/// frames between ticks, §1.5).
pub fn within(v: f32, a: f32, b: f32) -> bool {
    v >= a.min(b) && v <= a.max(b)
}

/// Test helpers shared by the tick-form tests: synthetic parameter maps
/// (a quadtree of four rows over a 400-unit square, so a camera moving
/// across it changes the sample) and a camera path.
#[cfg(test)]
pub(crate) mod testkit {
    use glam::Vec2;

    use crate::formats::hash::bin_hash;
    use crate::world::World;

    fn chunk(id: u32, body: &[u8]) -> Vec<u8> {
        let mut v = id.to_le_bytes().to_vec();
        v.extend_from_slice(&(body.len() as u32).to_le_bytes());
        v.extend_from_slice(body);
        v
    }

    /// One map chunk (0x8003B601): `rows` of equal length, a root node and
    /// four leaves, one row each (x < 0 / ≥ 0, y < 0 / ≥ 0).
    fn map(name: &str, rows: &[Vec<f32>]) -> Vec<u8> {
        let fields = rows[0].len() as u32;
        let stride = fields * 4;
        let mut h = Vec::new();
        h.extend(bin_hash(name).to_le_bytes());
        for f in [-200.0f32, -200.0, 200.0, 200.0] {
            h.extend(f.to_le_bytes());
        }
        for n in [5u32, rows.len() as u32, stride, fields] {
            h.extend(n.to_le_bytes());
        }
        let offsets: Vec<u8> = (0..fields).flat_map(|f| (f * 4).to_le_bytes()).collect();
        let values: Vec<u8> = rows.iter().flatten().flat_map(|v| v.to_le_bytes()).collect();
        let nodes: [[u16; 4]; 5] = [[1, 2, 3, 4], [0, 0, 0, 0], [0, 1, 0, 0], [0, 2, 0, 0], [0, 3, 0, 0]];
        let tree: Vec<u8> = nodes.iter().flatten().flat_map(|v| v.to_le_bytes()).collect();
        let mut body = chunk(0x0003_B602, &h);
        body.extend(chunk(0x0003_B604, &offsets));
        body.extend(chunk(0x0003_B605, &values));
        body.extend(chunk(0x0003_B608, &tree));
        chunk(0x8003_B601, &body)
    }

    /// A world with these maps (name, four rows).
    pub fn world(maps: &[(&str, [&[f32]; 4])]) -> World {
        let mut inner = Vec::new();
        for (name, rows) in maps {
            inner.extend(map(name, &rows.iter().map(|r| r.to_vec()).collect::<Vec<_>>()));
        }
        let bun = chunk(0x8003_B600, &inner);
        let mut w = World::default();
        w.parameter_maps = crate::formats::parameter_map::parse_parameter_maps(&bun).unwrap();
        w
    }

    /// The camera at tick `i` of 600: loops across the four quadrants at a
    /// speed that varies (some ticks still, some over 50 units).
    pub fn path(i: usize) -> Vec2 {
        let t = i as f32;
        let r = 120.0 + 60.0 * (t * 0.031).sin();
        let a = t * 0.023 + if i % 97 == 0 { 2.5 } else { 0.0 };
        if i % 41 == 7 { Vec2::new(-150.0, 170.0) } else { Vec2::new(r * a.cos(), r * a.sin()) }
    }

    /// α values for the frames between ticks.
    pub const ALPHAS: [f32; 6] = [1e-6, 0.125, 0.25, 0.5, 0.75, 0.999_999];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lerp_is_exact_at_zero_and_bounded_between() {
        for (a, b) in [(0.1f32, 0.7f32), (0.7, 0.1), (-3.0, 1e-7), (1e30, -1e30), (0.3, 0.3)] {
            assert_eq!(lerp(a, b, 0.0).to_bits(), a.to_bits());
            assert_eq!(lerp(a, b, f32::NAN).to_bits(), a.to_bits());
            assert_eq!(lerp(a, b, 1.0).to_bits(), b.to_bits());
            for k in 1..1000 {
                assert!(within(lerp(a, b, k as f32 / 1000.0), a, b));
            }
        }
    }
}
