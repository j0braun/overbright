use glam::{Mat4, Vec3};

use super::*;
use crate::formats::camera_tracks::{CameraEntry, CameraGroup, CameraKind};
use crate::opponents::GameRng;
use crate::draw::camera::Camera;

// --- The typed track cameras (docs/notes/parity/replay-cameras.md) -----------------

/// The typed camera kinds by their group key (the runtime table 0x7EC1F8,
/// 0x14 bytes {—, key, name, weight fn, flag fn}; **observed** filled under
/// Wine): the keys of InGameCommon.bun's typed groups (chunk 0x8003B202).
pub const TYPED_NOS: u32 = 0x0000_C9EF;
pub const TYPED_JUMP: u32 = 0x0018_7BBB;
pub const TYPED_SPEED: u32 = 0x03C7_EEB0;
pub const TYPED_CORNER: u32 = 0x5768_69E8;
pub const TYPED_BURNOUT: u32 = 0x057A_894E;
pub const TYPED_POWERSLIDE: u32 = 0x8140_9F9D;

/// What the weight functions read of the director's car.
#[derive(Clone, Copy, Debug, Default)]
pub struct TypedInput {
    /// The running nitrous shot's tank fraction (0x59FA40 non-zero →
    /// 0x5925C0), None without a shot.
    pub nitrous: Option<f32>,
    /// Car +0xB8 < 0: the third row's z of the car's matrix (+0x90), i.e.
    /// the car upside down (**inferred** from the layout; 0.9998 read on
    /// the road).
    pub upside_down: bool,
    /// 0x5ED230 (wheels down or a contact) and the flight prediction
    /// 0x5EC3E0(3.0, &height, &airtime, 1): height > 1 and airtime > 1.5.
    pub long_flight: bool,
    /// Car +0x42C (the exported forward speed) and +0x430 (the speed), m/s.
    pub forward_speed: f32,
    pub speed: f32,
    /// [car +0x10] +0x20, the racer info's +0x30 = the gearing top speed
    /// (traced; 0 = none → 60 m/s, 0x441C18).
    pub top: f32,
    /// 0x5EB320: the signed curvature (1 / m, 0x5D4780) of the car's last
    /// three logged points (its camera object +0x264); None without one.
    pub curvature: Option<f32>,
    /// 0x59FA20: the slip angle (rad, 0x591DA0) when the exported forward
    /// speed is 1 m/s or more, else 0.
    pub slip: f32,
    /// The sum of the four wheels' longitudinal slip (car +0x3B4 + 8i,
    /// r·ω − v, m/s) and the shown rpm (car +0x440), for Burnout.
    pub wheel_slip: f32,
    pub rpm: f32,
}

/// A typed group's weight for the car (0x44F9A0 calls the type's weight
/// fn; ≤ 0.1 counts as none). **Verified** in the disassembly
/// (0x441AF0..0x441F3A, constants read under Wine): NOS 0.5 + 0.25 × the
/// tank fraction while a shot runs; Jump 5 upside down or on a long flight;
/// Speed (forward speed over 10) 0.3 + 0.7 × forward / top, at most 1;
/// Corner (forward speed over 5) 20 × |curvature|, at most 1; PowerSlide
/// (speed over 4) (|slip| − 5°) × 1.4324 + 0.1 within 0..1 when over 5°.
/// Burnout (0x441D30, **verified**; constants read from the image):
/// with the forward speed within −10..20 m/s, clamp(sum of the wheels'
/// slip × 0.1 − 1, 0, 1), plus 10 × 1.1 / (1 + 1e-6) when the forward
/// speed is also within −1..1 and the rpm is over 3000 (0x7FBED8, 100 in
/// the image, is the rpm override: at 100 the car's +0x440 is read).
pub fn typed_weight(key: u32, c: &TypedInput) -> f32 {
    match key {
        TYPED_NOS => c.nitrous.map_or(0.0, |f| f * 0.25 + 0.5),
        TYPED_JUMP => {
            if c.upside_down || c.long_flight {
                5.0
            } else {
                0.0
            }
        }
        TYPED_SPEED => {
            if c.forward_speed <= 10.0 {
                return 0.0;
            }
            let top = if c.top > 0.0 { c.top } else { 60.0 };
            let w = c.forward_speed / top * 0.7 + 0.3;
            if w <= 0.0 { 0.0 } else { w.min(1.0) }
        }
        TYPED_CORNER => match c.curvature {
            Some(k) if c.forward_speed > 5.0 => {
                let w = k.abs() * 20.0;
                if w <= 0.0 { 0.0 } else { w.min(1.0) }
            }
            _ => 0.0,
        },
        TYPED_BURNOUT => {
            let v = c.forward_speed;
            if !(-10.0..=20.0).contains(&v) {
                return 0.0;
            }
            let w = c.wheel_slip * 0.1 - 1.0;
            let mut w = if w <= 0.0 { 0.0 } else { w.min(1.0) };
            if (-1.0..=1.0).contains(&v) && c.rpm > 3000.0 {
                w += 10.0 * 1.1 / (1.0 + 1e-6);
            }
            w
        }
        TYPED_POWERSLIDE => {
            if c.speed <= 4.0 {
                return 0.0;
            }
            let a = (c.slip.abs() - 0.087_266_46) * 1.432_394_5;
            if a <= 0.0 {
                return 0.0;
            }
            (a + 0.1).clamp(0.0, 1.0)
        }
        _ => 0.0,
    }
}

/// The type's flag fn (0x828649: while set the entries' y is mirrored,
/// 0x447A30 / 0x447A80): Corner 0x441D00 curvature < 0, PowerSlide 0x441F40
/// slip < 0; the others none (0x404800).
pub fn typed_flag(key: u32, c: &TypedInput) -> bool {
    match key {
        TYPED_CORNER => c.curvature.is_some_and(|k| k < 0.0),
        TYPED_POWERSLIDE => c.slip < 0.0,
        _ => false,
    }
}

/// An entry with its points' y mirrored (0x828649).
pub fn mirrored(e: &CameraEntry) -> CameraEntry {
    let mut m = e.clone();
    for p in m.eye.iter_mut().chain(m.look.iter_mut()) {
        p[1] = -p[1];
    }
    m
}

/// A typed shot: its group and track, when it began (s, the playback
/// clock) and the mirror flag.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypedShot {
    pub group: usize,
    pub track: usize,
    pub start: f32,
    pub mirror: bool,
}

/// The typed draw (0x44F9A0's second half, after the route groups): each
/// typed group's weight (≤ 0.1 none) summed; a group with weight gives its
/// tracks with an entry whose first is a camera (type ≠ 0) and, for a
/// weight under 10, that passes the visibility test 0x448280 at fraction 0
/// (with the flag's mirror) and is not one of the last three drawn (the
/// ring 0x828774); its weight shared among them. r = random(total) (all
/// groups' weights, including those left without a track) walks the
/// candidates: the first whose weight exceeds what is left of r. None when
/// the walk runs out.
#[allow(clippy::too_many_arguments)]
pub fn draw_typed(
    groups: &[CameraGroup],
    input: &TypedInput,
    recent: &[(usize, usize)],
    car: Mat4,
    view: &View,
    rng: &mut GameRng,
    blocked: &mut dyn FnMut(Vec3, Vec3) -> bool,
    now: f32,
) -> Option<TypedShot> {
    let weights: Vec<f32> = groups
        .iter()
        .map(|g| if g.kind == CameraKind::Typed { typed_weight(g.key, input) } else { 0.0 })
        .map(|w| if w <= 0.1 { 0.0 } else { w })
        .collect();
    let total: f32 = weights.iter().sum();
    if total <= 0.0 {
        return None;
    }
    let mut candidates: Vec<(TypedShot, f32)> = Vec::new();
    for (gi, g) in groups.iter().enumerate() {
        let w = weights[gi];
        if w <= 0.0 {
            continue;
        }
        let mirror = typed_flag(g.key, input);
        let first = candidates.len();
        for (ti, t) in g.tracks.iter().enumerate() {
            let Some(e) = t.entries.first() else { continue };
            if e.mover == 0 {
                continue;
            }
            if w < 10.0 {
                let e = if mirror { mirrored(e) } else { e.clone() };
                if !visible(&e, car, view, blocked) || recent.contains(&(gi, ti)) {
                    continue;
                }
            }
            candidates.push((TypedShot { group: gi, track: ti, start: now, mirror }, w));
        }
        let n = (candidates.len() - first) as f32;
        for c in &mut candidates[first..] {
            c.1 *= 1.0 / n;
        }
    }
    if candidates.is_empty() {
        return None;
    }
    let mut r = rng.unit(total);
    for (shot, w) in candidates {
        if r < w {
            return Some(shot);
        }
        r -= w;
    }
    None
}

/// A typed shot's camera at `now`: the fraction is the time since it began
/// over the track's duration (0x442B70); outside 0..1, or on an entry that
/// is not a camera, none (0x44FE80 → a new draw).
pub fn follow_typed(groups: &[CameraGroup], shot: &TypedShot, now: f32, car: Mat4) -> Option<Camera> {
    let track = groups.get(shot.group)?.tracks.get(shot.track)?;
    if track.duration <= 0.0 {
        return None;
    }
    let fraction = (now - shot.start) / track.duration;
    let i = entry_index(track, fraction)?;
    if track.entries[i].mover <= 1 {
        return None;
    }
    if shot.mirror {
        let mut t = track.clone();
        for e in &mut t.entries {
            *e = mirrored(e);
        }
        camera(&t, i, fraction, car)
    } else {
        camera(track, i, fraction, car)
    }
}
