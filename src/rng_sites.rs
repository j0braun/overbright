//! Native random draws whose values the clone does not use but whose stream
//! advance it must make, so the shared generator (0x7EA624) and the world
//! generator (world +0xF8) step as the original's do (
//! `docs/rng.md`, docs/native-fixes/N-RNG.tsv). Each routine names the
//! original's call sites; the values a routine returns are the original's
//! where the clone has a reader for them.

use crate::car::CarType;
use crate::formats::parts::PartDb;
use crate::opponents::GameRng;

/// The start-up's input binding (0x579560, its tail 0x5795B6..0x5795C3):
/// after the two input devices are bound, `Rand_Int(0x80000000)` on the
/// shared generator into 0x7FB2A0 (**traced**; its reader is **open**).
/// Once a process, after the views' lens set-up (0x57ED7C before
/// 0x57ED90 in the start-up chain 0x57ED20).
pub fn input_bind(rng: &mut GameRng) -> u32 {
    rng.below(0x8000_0000)
}

/// The traffic models' paint counts in the manager list's order (0x41AAF0:
/// every car type with record +0x844 = 2, in the type table's order, gets
/// a slot with a stock ride, 0x637050(0)): each the number of paint-slot
/// (0x3F) parts of any level for the type (0x631A30(type, 0x3F, any hash,
/// any level), the count 0x6371F0 draws below; 270 for every retail
/// traffic type, formats.md "Traffic").
pub fn traffic_paints(db: &PartDb, types: &[CarType]) -> Vec<u32> {
    types.iter().filter(|t| t.traffic).map(|t| db.parts_for(&t.name, 0x3F).len() as u32).collect()
}

/// The traffic manager's slot reset 0x412840 (**traced**), per slot in
/// list order: 0x6371F0's paint, `Rand_Int(count)` on the shared
/// generator (0x63720F; none when the type has no paint), then
/// `Rand_Int(0xFFFF)` on the shared generator (0x41289C) and, with a world
/// built ([0x890080] ≠ 0), `Rand_Int(0xFFFF)` on the world generator in its
/// place (0x4128B7). That value seeds 0x61BC30's skin pick on a local state
/// (0x43C140 at 0x61BC71), which moves neither generator; the skin byte and
/// the paint do not show on a traffic car (formats.md "Traffic"), so the
/// clone keeps only the draws.
///
/// Called by the first set-up of a process (0x4FE9F0 → 0x41AAF0, once:
/// flag +0x90C; no world yet, **measured** 15 + 15 before the main menu)
/// and by every world build (0x4179F0, with the world: 15 × 3).
pub fn model_slots(rng: &mut GameRng, mut world: Option<&mut GameRng>, paints: &[u32]) {
    for &count in paints {
        if count != 0 {
            rng.below(count);
        }
        rng.below(0xFFFF);
        if let Some(w) = world.as_deref_mut() {
            w.below(0xFFFF);
        }
    }
}

/// The world generator's seed (0x5FF557 in the world object's constructor
/// 0x5FF4A0): `Rand_Int(0x7FFFFFFF)` on the shared generator, stored as
/// the world's state +0xF8 (**traced**; **measured** one draw per world
/// build: free-roam entry, race load, restart).
pub fn world_seed(rng: &mut GameRng) -> GameRng {
    GameRng(rng.below(0x7FFF_FFFF))
}

/// The camera director's live draw (0x5FB060 at 0x5FB2EA): `Rand_Float(1.0)`
/// on the shared generator once a world update while its gate holds (the
/// player's NIS scene +0x234 set: a race's start scene through the
/// countdown until the scene ends past the green, a shop scene). The value
/// picks among the director's cameras, which the clone's scene camera
/// stands in for, so only the draw is kept (**traced**; **measured** c1:
/// every frame of the start scene and countdown, 312 frames after the
/// green, 140 in a shop drive-in).
pub fn director(rng: &mut GameRng) {
    rng.unit(1.0);
}

/// The collision hull's support (0x5AFBB0 at 0x5AFBF1): one
/// `Rand_Int(0xFFFF)` on the shared generator at the top of every call,
/// plus the high-resolution counter (0x43BDF0), as the call's visit stamp
/// (compared with each vertex record's +0xE to reuse its dot +0x20 within
/// the call). The climb's start is the side's last reported vertex, not
/// this value, so it does not reach the contact (`narrow`); only the
/// stream advance is native. For the collision code to call once per
/// support taken (docs/native-fixes/N-RNG.tsv; not wired: the clone's
/// shared generator already misses other draws, docs/rng.md).
pub fn hull_support_start(rng: &mut GameRng, vertices: usize) -> usize {
    let r = rng.below(0xFFFF) as usize;
    if vertices == 0 { 0 } else { r % vertices }
}

/// 0x40D5C0, the honk / light flash a car proposes at the human's car
/// (**verified** in the decompile): called
/// right before the constructor of "Avoid Cars as a traffic car" (0x426CFF,
/// 0x426E4F), IJustGotHit (0x42D1F5, 0x42D7F5, 0x42DB59) and AvoidAvoidable
/// (0x42D274, 0x42D68C, 0x42D874, 0x42DBD8 — before the controller's
/// vt+0x9C acceptance). Only for the human's car (0x614890; not a traffic
/// object 0x6148D0, not a car type with record +0x844 = 1, 0x60BED0 — no
/// stock car); then by the own car's kind (car +0x24): traffic (4) always,
/// an AI racer (3; race rivals and encounter racers, 0x4260B0 / 0x53F010)
/// only when 0x403820 keeps the human's entry (not wholly behind: its gap
/// over −(both +0x0C) − 0.5 m), any other kind never. Its stores (car
/// +0x926..+0x938) are read only past a return 0x5F61A0 always takes
/// (+0x925 is only ever written 0), so nothing is heard or seen: what is
/// ported are the draws on the world generator (0x5EA510 on [0x890080]
/// +0xF8), in order: F(0.4), F(0.2), F(0.4), F(1.0) (the timings and the
/// active flag), F(1.0) under p for the horn, and F(1.0) for the flash only
/// when the horn was drawn. p = 1 when the human's entry +0x04 (its
/// velocity along the car's direction less the car's) is over 50 m/s, or
/// for a traffic controller (car +0x14 → +4 = 3) under −3 m/s; else 0.75
/// (the entry's object is the human; the 0.05 / 0.075 odds for other
/// objects cannot be reached past the gate).
pub fn honk(rng: &mut GameRng, closing: f32, traffic: bool) {
    rng.unit(0.4);
    rng.unit(0.2);
    rng.unit(0.4);
    rng.unit(1.0);
    let p = if closing > 50.0 || (traffic && closing < -3.0) { 1.0 } else { 0.75 };
    if rng.unit(1.0) < p {
        rng.unit(1.0);
    }
}

#[cfg(test)]
mod tests {
    /// 0x40D5C0's draws: four, then the horn's under p, then the
    /// flash's only after a horn — 6 draws at p = 1, 5 or 6 at 0.75.
    #[test]
    fn the_honk_draws_four_then_the_horn_then_the_flash() {
        let count = |closing: f32, traffic: bool, seed: u32| {
            let mut a = GameRng(seed.wrapping_mul(2_654_435_761));
            let mut b = a.clone();
            honk(&mut a, closing, traffic);
            (0..8).position(|_| {
                let same = a.0 == b.0;
                b.unit(1.0);
                same
            })
        };
        // p = 1 (closing over 50 m/s, or a traffic car closing under −3).
        for seed in 1..20 {
            assert_eq!(count(60.0, false, seed), Some(6));
            assert_eq!(count(-4.0, true, seed), Some(6));
        }
        // p = 0.75: five draws when the horn is not drawn, six when it is.
        let n: Vec<_> = (1..200).map(|s| count(0.0, false, s)).collect();
        assert!(n.iter().all(|&k| k == Some(5) || k == Some(6)));
        assert!(n.contains(&Some(5)) && n.contains(&Some(6)));
        // A racer closing under −3 m/s is not a traffic controller: 0.75.
        let m: Vec<_> = (1..200).map(|s| count(-4.0, false, s)).collect();
        assert!(m.contains(&Some(5)));
    }

    use super::*;

    /// 0x412840 (0x63720F, 0x41289C, 0x4128B7): paint, shared 0xFFFF, then
    /// with a world the world's 0xFFFF, slot by slot.
    #[test]
    fn model_slots_draw_paint_shared_then_world_per_slot() {
        let paints = [270, 270, 0];
        let (mut a, mut w) = (GameRng::default(), GameRng(7));
        model_slots(&mut a, Some(&mut w), &paints);
        let (mut b, mut v) = (GameRng::default(), GameRng(7));
        for &n in &paints {
            if n != 0 {
                b.below(n);
            }
            b.below(0xFFFF);
            v.below(0xFFFF);
        }
        assert_eq!((a.0, w.0), (b.0, v.0));
        // At start-up (no world): two shared draws a slot with paint.
        let mut c = GameRng::default();
        model_slots(&mut c, None, &[270; 15]);
        let mut d = GameRng::default();
        for _ in 0..30 {
            d.below(1);
        }
        assert_eq!(c.0, d.0, "30 shared draws");
    }

    /// 0x5FF557: the world's state is the shared draw itself (no step).
    #[test]
    fn the_world_seed_is_the_shared_draw() {
        let mut a = GameRng::default();
        let w = world_seed(&mut a);
        assert_eq!(w.0, 0x1234_5678 % 0x7FFF_FFFF);
        assert_ne!(a.0, 0x1234_5678);
    }

    /// 0x5AFBF1: one shared draw a query.
    #[test]
    fn the_hull_support_start_draws_once() {
        let mut a = GameRng::default();
        let i = hull_support_start(&mut a, 12);
        let mut b = GameRng::default();
        assert_eq!(i, b.below(0xFFFF) as usize % 12);
        assert_eq!(a.0, b.0);
    }
}
