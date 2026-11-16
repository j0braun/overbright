//! Rate independence (docs/uncapped-fps.md "Drawn objects between ticks"): the
//! particles a drawn frame shows between two ticks, a pure function of the
//! two ticks' particle lists and α. The simulation steps the particles on
//! the tick only (0x492E10's update, once per 60 fps frame in the
//! original); a frame between ticks pairs them by [`Particle::id`]:
//!
//! * α ≤ 0: S_{n−1}'s list as it is (the older tick's picture, bit for
//!   bit, the ones that die in tick n included); α ≥ 1: S_n's.
//! * A particle in both: S_n's particle at the blend of its two positions
//!   (or where `carry` puts it: a particle carried with its owner car is
//!   placed with the car's drawn pose) with its life (the colour and size
//!   curves' age) blended; its flip-book frame held at S_{n−1} (a
//!   discrete value). Over [`JUMP`] apart (a reused id, a restore): S_n.
//! * A particle born in tick n: at its S_n state, its size × α (it grows
//!   in over the frames before the tick that shows it whole).
//! * A particle that died in tick n: not drawn between the ticks.
use super::*;

/// A move between two ticks beyond which a particle is not blended (m),
/// as the cars' (`interpolate::JUMP`).
pub const JUMP: f32 = 30.0;

/// The drawn particles (`out`, in S_n's order) and their size factors
/// (`scale`: empty = all 1) `alpha` of the way from `prev` (S_{n−1}) to
/// `cur` (S_n). `carry(older, newer)` gives the drawn position of a
/// particle its owner carries (None: the world-space blend of the two
/// positions, or S_n's for a new one). `out` and `scale` are cleared first.
pub fn between(
    prev: &[Particle],
    cur: &[Particle],
    alpha: f32,
    carry: impl Fn(Option<&Particle>, &Particle) -> Option<Vec3>,
    out: &mut Vec<Particle>,
    scale: &mut Vec<f32>,
) {
    out.clear();
    scale.clear();
    // `!(alpha > 0)` also catches NaN.
    if !(alpha > 0.0) {
        out.extend_from_slice(prev);
        return;
    }
    if alpha >= 1.0 {
        out.extend_from_slice(cur);
        return;
    }
    use crate::render::native::fx::tick::lerp;
    // Spawned in id order and kept in order (`retain`), so S_{n−1} is
    // sorted by id; a list put together otherwise is searched linearly.
    let sorted = prev.windows(2).all(|w| w[0].id < w[1].id);
    let find = |id: u32| -> Option<&Particle> {
        if sorted {
            prev.binary_search_by_key(&id, |p| p.id).ok().map(|i| &prev[i])
        } else {
            prev.iter().find(|p| p.id == id)
        }
    };
    out.reserve(cur.len());
    scale.reserve(cur.len());
    for c in cur {
        let mut d = *c;
        match find(c.id).filter(|p| p.description == c.description && p.position.distance(c.position) <= JUMP) {
            Some(p) => {
                d.position = carry(Some(p), c).unwrap_or_else(|| {
                    Vec3::new(
                        lerp(p.position.x, c.position.x, alpha),
                        lerp(p.position.y, c.position.y, alpha),
                        lerp(p.position.z, c.position.z, alpha),
                    )
                });
                d.life = lerp(f32::from(p.life), f32::from(c.life), alpha).round() as i16;
                d.frame = p.frame;
                scale.push(1.0);
            }
            None if find(c.id).is_some() => scale.push(1.0),
            None => {
                if let Some(at) = carry(None, c) {
                    d.position = at;
                }
                scale.push(alpha);
            }
        }
        out.push(d);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn particle(id: u32, x: f32, life: i16) -> Particle {
        Particle {
            description: 0,
            palette: 0,
            key: 0,
            life,
            angle: 0,
            speed: 0,
            carried: 0,
            position: Vec3::new(x, 1.0, 2.0),
            direction: [0; 3],
            frame: id as u8,
            id,
        }
    }

    fn bits(p: &Particle) -> ([u32; 3], i16, u8, u32) {
        (p.position.to_array().map(f32::to_bits), p.life, p.frame, p.id)
    }

    /// α = 0: S_{n−1} as it is (the one dying in tick n too); α = 1: S_n;
    /// α = 0.5: the survivors halfway, the newborn at S_n half size, the
    /// dead gone; a survivor that jumped is drawn at S_n.
    #[test]
    fn ends_are_the_ticks_and_between_pairs_by_id() {
        let prev = [particle(1, 0.0, 1000), particle(2, 10.0, 500), particle(3, 0.0, 10), particle(4, 0.0, 900)];
        let mut cur = vec![particle(1, 1.0, 980), particle(2, 12.0, 480), particle(4, 100.0, 880), particle(5, 7.0, 1024)];
        cur[0].frame = 9;
        let (mut out, mut scale) = (Vec::new(), Vec::new());
        between(&prev, &cur, 0.0, |_, _| None, &mut out, &mut scale);
        assert_eq!(out.iter().map(bits).collect::<Vec<_>>(), prev.iter().map(bits).collect::<Vec<_>>());
        assert!(scale.is_empty());
        between(&prev, &cur, 1.0, |_, _| None, &mut out, &mut scale);
        assert_eq!(out.iter().map(bits).collect::<Vec<_>>(), cur.iter().map(bits).collect::<Vec<_>>());
        between(&prev, &cur, 0.5, |_, _| None, &mut out, &mut scale);
        assert_eq!(out.iter().map(|p| p.id).collect::<Vec<_>>(), [1, 2, 4, 5], "the dead one (3) is not drawn");
        assert_eq!(out[0].position.x, 0.5);
        assert_eq!(out[0].life, 990);
        assert_eq!(out[0].frame, 1, "the frame held at S_{{n−1}}");
        assert_eq!(out[1].position.x, 11.0);
        assert_eq!(out[2].position.x, 100.0, "a jump over 30 m: S_n");
        assert_eq!(out[3].position.x, 7.0, "a newborn at S_n");
        assert_eq!(scale, [1.0, 1.0, 1.0, 0.5], "… growing in");
        // Every coordinate within the two ticks' at any α.
        for k in 1..100 {
            let a = k as f32 / 100.0;
            between(&prev, &cur, a, |_, _| None, &mut out, &mut scale);
            assert!(out[0].position.x >= 0.0 && out[0].position.x <= 1.0);
            assert!(out[1].position.x >= 10.0 && out[1].position.x <= 12.0);
            assert_eq!(scale[3], a);
        }
    }

    /// The carried particles take `carry`'s place, a newborn too.
    #[test]
    fn carried_particles_take_the_owner_place() {
        let prev = [particle(1, 0.0, 1000)];
        let cur = [particle(1, 1.0, 980), particle(2, 1.0, 1000)];
        let (mut out, mut scale) = (Vec::new(), Vec::new());
        between(&prev, &cur, 0.25, |p, c| Some(Vec3::splat(if p.is_some() { 5.0 } else { 6.0 } + c.id as f32)), &mut out, &mut scale);
        assert_eq!(out[0].position, Vec3::splat(6.0));
        assert_eq!(out[1].position, Vec3::splat(8.0));
    }

    /// Spawns count the ids up; the update keeps their order.
    #[test]
    fn spawns_number_the_particles_in_order() {
        let mut d = Description::default();
        d.life = [1.0, 0.0];
        d.rate = [600.0, 0.0];
        d.range = 1e9;
        let lib = Library { descriptions: vec![d], ..Default::default() };
        let mut sys = System::default();
        let mut e = Emitter::new(0, Mat4::IDENTITY, 7);
        e.on = true;
        for _ in 0..30 {
            sys.emit(&lib, &mut e, 1.0 / 60.0);
            sys.update(&lib, 1.0 / 60.0, |_| None);
        }
        assert!(sys.particles.len() > 5);
        assert!(sys.particles.windows(2).all(|w| w[0].id < w[1].id));
        assert_eq!(sys.next_id, sys.particles.last().unwrap().id + 1);
    }
}
