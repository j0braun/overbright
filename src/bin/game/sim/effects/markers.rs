use crate::*;

/// The marker effects (table 0x802300, 12 bytes an entry: kind, id, name;
/// 0x605690 gives each object the emitter descriptions of the group
/// `bin_hash(name)`, 0x48B8B0). Kind 0 shows the nearest marker table
/// entry whose type (0x500DE0) is the id, kind 1 the nearest game tip,
/// kind 2 the nearest bank (career +0x600 and +0x800 pointers: the
/// unsent engage tips, 0x528990, and the open cash pickups, 0x5141B0).
pub(crate) const MARKER_EFFECTS: [(u8, u8, &str); 12] = [
    (0, 0, "XEE_EVENT_CIRCUIT"),
    (0, 1, "XEE_EVENT_DRAG"),
    (0, 2, "XEE_EVENT_STREET"),
    (0, 3, "XEE_EVENT_DRIFT"),
    (0, 4, "XEE_EVENT_SPRINT"),
    (0, 5, "XEE_EVENT_URL"),
    (0, 6, "XEE_SHOWCASE"),
    (0, 15, "XEE_WORLD_CHALLENGE"),
    (0, 7, "XEE_EVENT_SUV"),
    (0, 8, "XEE_DVD"),
    (1, 0, "XEE_GAME_TIP"),
    (2, 0, "XEE_BANK"),
];

impl Effects {
    /// Put each marker effect where it is wanted, or off (0x6054C0 per
    /// slot, 0x602230 when a slot is empty or its spot has no ground). One
    /// already on stays while it is within 0.001 of the spot on each axis
    /// (0x43D190); else it goes and starts again there: its group's
    /// emitters, on (+0x10 = 1), at the identity turned to the spot
    /// (0x43C780, 0x4012B0), the spot's z, or the ground under it when that
    /// z is 0 (0x5E72F0; none, and it stays off). The spot is kept as
    /// given, z 0 included.
    pub(crate) fn set_markers(
        &mut self,
        library: &nfsu2::particles::Library,
        wanted: [Option<Vec3>; 12],
        ground: impl Fn(Vec3) -> Option<f32>,
    ) {
        for (k, want) in wanted.into_iter().enumerate() {
            let slot = &mut self.markers[k];
            if let (Some((at, _)), Some(w)) = (slot.as_ref(), want)
                && (*at - w).abs().max_element() <= 0.001
            {
                continue;
            }
            *slot = None;
            let Some(w) = want else { continue };
            let z = if w.z == 0.0 {
                match ground(w) {
                    Some(z) => z,
                    None => continue,
                }
            } else {
                w.z
            };
            let transform = glam::Mat4::from_translation(Vec3::new(w.x, w.y, z));
            if debug_env().debug_markers {
                eprintln!("marker {} at {w:?} ground {:?}", MARKER_EFFECTS[k].2, ground(w));
                for i in library.group(nfsu2::formats::hash::bin_hash(MARKER_EFFECTS[k].2)) {
                    let d = &library.descriptions[i];
                    eprintln!(
                        "  emitter {} orient {} offset {:?} extent {:?} speed {:?} gravity {}",
                        d.name, d.orientation, d.offset, d.extent, d.speed, d.gravity
                    );
                }
            }
            let emitters = library
                .group(nfsu2::formats::hash::bin_hash(MARKER_EFFECTS[k].2))
                .into_iter()
                .map(|d| nfsu2::particles::Emitter::new(d, transform, (self.rng.next() * 4.0e9) as u32))
                .collect();
            *slot = Some((w, emitters));
        }
    }

    /// 0x433090's emitter half, per flare (0x4335C0 calls it for each of
    /// the flag girl's two flares every character update): every emitter
    /// of the group goes to the flare's tip (an identity matrix with the
    /// tip as its translation, 0x43C780 / 0x4012B0) with +0x80 = 2 ×
    /// unit(the flare's vector) (0x43CF10) and fires a burst of −1.0
    /// (0x491E50(e, −1.0, 0)).
    pub(crate) fn flag_girl(&mut self, library: &nfsu2::particles::Library, flares: &[(Vec3, Vec3)]) {
        if flares.is_empty() {
            return;
        }
        if self.flag_girl.is_empty() {
            for d in library.group(nfsu2::formats::hash::bin_hash("XECH_FLAG_GIRL_FLARE")) {
                let seed = (self.rng.next() * 4.0e9) as u32;
                self.flag_girl.push(nfsu2::particles::Emitter::new(d, glam::Mat4::IDENTITY, seed));
            }
        }
        for &(tip, v) in flares {
            for e in &mut self.flag_girl {
                e.transform = glam::Mat4::from_translation(tip);
                e.velocity = v.normalize_or_zero() * 2.0;
                self.system.emit(library, e, -1.0);
            }
        }
    }
}
