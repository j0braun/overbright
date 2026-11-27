//! The impact sounds' events the physics posts (`drive::ContactSounds`): an impact's kind and volume byte, the
//! repeat and play tests, and the queue the audio drains. No playback (`collision` plays them).

use glam::Vec3;

/// The volume curve's scale by kind (0x7884F4, kinds 0–8).
pub const SCALE: [f32; 9] = [18.0, 2.5, 15.0, 1.0, 20.0, 8.0, 1.0, 2.5, 2.5];

/// The volume byte for the summed impulse of a kind (0x479A20).
pub fn volume(impulse: f32, kind: usize) -> u8 {
    let hi = 3.0 * SCALE[kind.min(8)];
    let t = impulse / hi;
    if t > 0.0 { (127.0 * t.min(1.0)) as u8 } else { 0 }
}

/// Class 0's volume (0x47A700): + 50, at most 127, for a car tipped (up z
/// under 0.9) without all four wheels down.
pub fn tipped(volume: u8, up_z: f32, wheels_down: usize) -> u8 {
    if up_z < 0.9 && wheels_down < 4 { (volume as u32 + 50).min(127) as u8 } else { volume }
}

/// Class 0's repeat filter (0x46ABA0): drop the sound when a live sound of
/// the same car along a wall (kind 5) started from a push within 0.8 (dot)
/// of this one's direction.
pub fn repeats_wall(dir: Vec3, live: impl IntoIterator<Item = (usize, Vec3)>) -> bool {
    live.into_iter().any(|(kind, d)| kind == 5 && dir.dot(d) > 0.8)
}

/// Class 2's play test (0x46A670): a car with a sound object in the pair
/// (every clone car, inferred) or volume ≥ 10, and the prop not already
/// sounding.
pub fn prop_plays(kind: usize, volume: u8, sounding: bool) -> bool {
    (kind == 7 || volume >= 10) && !sounding
}

/// The car-against-car kind (0x46A770): 1 (a sideswipe) when the cars
/// head within 45° of each other (or opposite) and the point is off the
/// nose or tail line of either (|forward · arm| ≤ 0.8), else 0.
pub fn car_kind(fwd_a: Vec3, pos_a: Vec3, fwd_b: Vec3, pos_b: Vec3, point: Vec3) -> usize {
    let (fa, fb) = (fwd_a.truncate().normalize_or_zero(), fwd_b.truncate().normalize_or_zero());
    if fa.dot(fb).abs() > 0.707 {
        let side = |f: Vec3, p: Vec3| f.normalize_or_zero().dot((point - p).normalize_or_zero()).abs() <= 0.8;
        if side(fwd_a, pos_a) || side(fwd_b, pos_b) {
            return 1;
        }
    }
    0
}

/// The car-against-track kind (0x46AA20) from the car's forward and up
/// axes and the contact normal: a wall (|n.z| ≤ 0.707) is 5 when the car
/// runs along it (|forward · n| ≤ 0.707), else 4; on a floor 2 with the
/// car on its side or roof (up.z < 0.1), 3 on its wheels (up.z > 0.707),
/// else 4.
pub fn track_kind(forward: Vec3, up: Vec3, normal: Vec3) -> usize {
    let n = normal.normalize_or_zero();
    if n.z.abs() <= 0.707 {
        return if forward.dot(n).abs() <= 0.707 { 5 } else { 4 };
    }
    if up.z < 0.1 {
        2
    } else if up.z > 0.707 {
        3
    } else {
        4
    }
}

/// One impact to play: where and what (`material` for the props' kinds 7
/// and 8).
#[derive(Clone, Copy, Debug)]
pub struct Impact {
    pub at: Vec3,
    pub kind: usize,
    pub volume: u8,
    pub material: usize,
    /// The track sample's +60 (0x47A770, **verified**): the impact's car
    /// (object +0x118) is a player's (+0x18) whose camera event kind
    /// (+0x58) is 6, the crash camera.
    pub crash_camera: bool,
    /// A water drum's splash rides on this impact's sound object
    /// (0x46AE00, vtable 0x78BB08 slot 8: the prop object's start, called
    /// by the queue's flush 0x481850 at 0x4818A6, so it is capped with the
    /// object; see [`super::collision::splash_level`]).
    pub splash: bool,
}

/// Impacts posted by the physics, waiting for the audio (bounded: a run
/// with no listener drops the oldest): a session's own queue (the
/// physics posts into the one it is handed, `drive::ContactSounds`), no
/// longer one process-wide queue.
#[derive(Clone, Debug, Default)]
pub struct Pending(Vec<Impact>);

impl Pending {
    /// A record's first push: post its sound at a volume byte.
    pub fn post_volume(&mut self, at: Vec3, kind: usize, volume: u8, material: usize) {
        self.post_impact(Impact { at, kind, volume, material, crash_camera: false, splash: false });
    }

    /// Post an impact as built.
    pub fn post_impact(&mut self, impact: Impact) {
        let p = &mut self.0;
        if p.len() >= 64 {
            p.remove(0);
        }
        p.push(impact);
    }

    /// A record's first push: post its sound.
    pub fn post(&mut self, at: Vec3, kind: usize, impulse: f32) {
        self.post_volume(at, kind, volume(impulse, kind), 0);
    }

    /// Take the posted impacts.
    pub fn drain(&mut self) -> Vec<Impact> {
        std::mem::take(&mut self.0)
    }

    /// The impacts posted, oldest first.
    pub fn as_slice(&self) -> &[Impact] {
        &self.0
    }
}
