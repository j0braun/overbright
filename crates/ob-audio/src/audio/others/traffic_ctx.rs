use super::doppler::Doppler;
use super::horn::Horn;
use super::stereo::Stereo;
use crate::audio::stitch::StitchVoice;

/// The objects of one traffic sound context (mask 0x1F: Engine, Woosh,
/// Horn, Stereo, BottomOut). They live with the context, not with a car:
/// the traffic manager hands the context from car to car (0x463E30).
#[derive(Default)]
pub(super) struct Traffic {
    pub(super) event: Option<u32>,
    pub(super) variant: i32,
    pub(super) doppler: Doppler,
    pub(super) whoosh: Option<StitchVoice>,
    pub(super) last_whoosh: Option<f32>,
    pub(super) whoosh_started: bool,
    pub(super) horn: Horn,
    /// CARSFX_TrafficBottomOut (0x47F2B0 → 0x47E980).
    pub(super) bottom: crate::audio::roadwind::Bottom,
    pub(super) stereo: Stereo,
}

/// A traffic sound context (vtable 0x78BA78, 0x48 bytes, ctor 0x4795D0):
/// +0x30 the car, +0x3C attached.
#[derive(Default)]
pub(super) struct Context {
    /// The car it is attached to (the clone's traffic pool slot).
    pub(super) car: Option<usize>,
    pub(super) t: Traffic,
}

/// 0x463E30: a traffic car's sound context stays attached while the car
/// is within 70 m (0x78A414) of the nearest player's car (0x459A50).
pub const CONTEXT_RANGE: f32 = 70.0;

/// 0x476AF0 (the traffic manager's slot 2): one context per traffic car
/// of the car table (car +0x14 → +4 = 3), at most 6 (also the manager's
/// cap +0x14, set by 0x463AD0(3, 6) at 0x4857A9).
pub const TRAFFIC_CONTEXTS: usize = 6;

/// 0x463E30 over the manager's car slots (+0x20..+0x5C, in the order the
/// cars were added, 0x463F10): a car with a context releases it at 70 m
/// or more; a car within 70 m without one takes the first free context
/// of the list (0x464350: +0x3C clear), if any — no priority, no
/// stealing. `cars` are (key, distance) in slot order; `contexts` the
/// contexts' cars. Returns (context, key) attaches and context releases.
pub fn assign(contexts: &[Option<usize>], cars: &[(usize, f32)]) -> (Vec<(usize, usize)>, Vec<usize>) {
    let mut held: Vec<Option<usize>> = contexts.to_vec();
    let (mut attach, mut release) = (Vec::new(), Vec::new());
    // Contexts whose car is no longer driven (the clone's pool slot lost
    // its lane) let go (**inferred**: the native car stays in its slot).
    for (c, k) in held.iter_mut().enumerate() {
        if let Some(key) = *k
            && !cars.iter().any(|&(j, _)| j == key)
        {
            *k = None;
            release.push(c);
        }
    }
    for &(key, d) in cars {
        if let Some(c) = held.iter().position(|&k| k == Some(key)) {
            if d >= CONTEXT_RANGE {
                held[c] = None;
                release.push(c);
            }
        } else if d < CONTEXT_RANGE
            && let Some(c) = held.iter().position(|k| k.is_none())
        {
            held[c] = Some(key);
            attach.push((c, key));
        }
    }
    (attach, release)
}
