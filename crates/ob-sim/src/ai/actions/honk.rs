use super::*;

/// An AI racer's 0x40D5C0 on the entry of object `id` (own kind 3: the
/// human's entry kept by 0x403820).
pub(super) fn racer_honk(rng: &mut GameRng, list: &[Entry], own: usize, id: usize) {
    let Some(e) = list.iter().find(|e| e.car.id == id) else { return };
    if !e.car.human || e.car.traffic {
        return;
    }
    let mine = &list[own];
    if gap(list, own, e) <= -(mine.along + e.along) - 0.5 {
        return;
    }
    crate::rng_sites::honk(rng, e.closing, false);
}
