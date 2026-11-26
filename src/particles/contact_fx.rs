use super::*;

/// The surface table's contact effects (0x800EE0, 0x98 bytes each; 0x5E0630
/// hashes the names into the lists 0x5DC4D0 walks): per surface the group
/// and the 0..1 share's range for event 5 (the push |J|, +0x80 / +0x84 /
/// +0x88) and event 6 (the push's sliding speed, +0x8C / +0x90 / +0x94).
pub struct SurfaceEffects {
    pub impulse: Option<(&'static str, f32, f32)>,
    pub closing: Option<(&'static str, f32, f32)>,
}

/// The surfaces with contact effects; every other has none.
pub fn surface_effects(surface: u8) -> SurfaceEffects {
    let (impulse, closing) = match surface {
        1 => (Some(("XEBO_ROAD", 10.0, 15.0)), Some(("XECS_SOLID_WALL", 5.0, 50.0))),
        15 => (Some(("XECS_GRASS", 15.0, 100.0)), Some(("XECS_GRASS", 15.0, 50.0))),
        17 => (Some(("XECS_WOOD_WALL", 4.0, 15.0)), Some(("XECS_WOOD_WALL", 15.0, 50.0))),
        18 => (Some(("XECS_PLASTIC_WALL", 4.0, 15.0)), Some(("XECS_PLASTIC_WALL", 15.0, 50.0))),
        19 => (Some(("XECS_GLASS_WALL", 4.0, 15.0)), Some(("XECS_GLASS_WALL", 15.0, 50.0))),
        20 => (Some(("XECS_SOLID_WALL", 10.0, 15.0)), Some(("XECS_SOLID_WALL", 15.0, 50.0))),
        28 => (Some(("XECI_CAR", 10.0, 15.0)), None),
        _ => (None, None),
    };
    SurfaceEffects { impulse, closing }
}

/// The share 0x5DC4D0 hands the emitters: (x − lo) / (hi − lo), 0 when the
/// span is under 1e−5, clamped 0..1; `None` when x does not pass lo.
pub fn share(x: f32, lo: f32, hi: f32) -> Option<f32> {
    if x <= lo {
        return None;
    }
    let span = hi - lo;
    Some(if span < 1e-5 { 0.0 } else { ((x - lo) / span).clamp(0.0, 1.0) })
}

/// A contact effect posted by the physics: a group burst at a point.
#[derive(Clone, Copy, Debug)]
pub struct Burst {
    pub group: u32,
    pub at: Vec3,
    pub velocity: Vec3,
    pub share: f32,
    /// The push behind it (clone only, read by the Sparks & Fire tier;
    /// the native burst uses none of it): the sliding speed (m/s), |J|
    /// and the contact normal.
    pub slide: f32,
    pub impulse: f32,
    pub normal: Vec3,
}

pub(super) static PENDING: std::sync::Mutex<Vec<Burst>> = std::sync::Mutex::new(Vec::new());

/// 0x5A9630's effects for one push on a contact record with `surface`
/// (the pair's, 0x592C20): event 6 when the push's sliding speed (its
/// third argument, record +0x40: `drive::slide_speed`) passes its low
/// bound, event 5 when |J| passes its own, each with the faster body's
/// velocity. (The game also needs the point within 50 m of the view,
/// 0x5F0FC0; the clone's spawn range test stands in.)
pub fn post_contact(surface: u8, at: Vec3, slide: f32, impulse: f32, velocity: Vec3, normal: Vec3) {
    let fx = surface_effects(surface);
    let mut bursts = Vec::new();
    if let Some((group, lo, hi)) = fx.closing {
        if let Some(s) = share(slide, lo, hi) {
            bursts.push(Burst {
                group: crate::formats::hash::bin_hash(group),
                at,
                velocity,
                share: s,
                slide,
                impulse,
                normal,
            });
        }
    }
    if let Some((group, lo, hi)) = fx.impulse {
        if let Some(s) = share(impulse, lo, hi) {
            bursts.push(Burst {
                group: crate::formats::hash::bin_hash(group),
                at,
                velocity,
                share: s,
                slide,
                impulse,
                normal,
            });
        }
    }
    if let Ok(mut p) = PENDING.lock() {
        if p.len() + bursts.len() > 256 {
            p.clear();
        }
        p.extend(bursts);
    }
}

/// Take the posted bursts.
pub fn drain() -> Vec<Burst> {
    PENDING.lock().map(|mut p| std::mem::take(&mut *p)).unwrap_or_default()
}

/// The contact surface a pair takes (0x592C20): each body's surface, the
/// one whose table float +0x34 is larger (car 28: 0; the track's road and
/// walls 0.1, grass 1, wood/plastic/glass 0.5). Ties keep the first.
pub fn pair_surface(a: u8, b: u8) -> u8 {
    static WEIGHT: crate::exe_tables::Table<[f32; 29]> = crate::exe_tables::Table::new(|t| &t.surface_weight);
    let w = |s: u8| WEIGHT.get(s as usize).copied().unwrap_or(0.0);
    if w(b) > w(a) { b } else { a }
}

/// The car's surface index in the table (28, `XECI_CAR`'s row).
pub const CAR_SURFACE: u8 = 28;

/// Fire a burst: every description of the group, each as a one-off
/// emitter at the point (identity orientation, 0x5DC4D0 sets only the
/// translation and +0x80) with −share.
pub fn burst(system: &mut System, library: &Library, b: &Burst, seed: &mut Rng) {
    for d in library.group(b.group) {
        let mut e = Emitter::new(d, Mat4::from_translation(b.at), (seed.next() * 4.0e9) as u32);
        e.velocity = b.velocity;
        system.emit(library, &mut e, -b.share);
    }
}
