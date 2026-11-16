//! Impact sounds: what a contact record plays on its first push, traced
//! from the collision sound objects the audio manager keeps per record.
//!
//! * A contact record's constructor (0x5A9490 → 0x483E20 → 0x481CD0) makes
//!   one sound object for it (none while 10 play requests are queued this
//!   update, `9 < +0xC` returns; at most 5 play, [`Pool`]): by
//!   0x459680 class 2 when a prop is in the pair (0x592D60, vtable
//!   0x78BB08), 1 when both bodies are cars (0x592CA0, 0x78BE90), else 0
//!   (a car against the track, 0x78BEE0).
//! * A push on the record (0x5A9630) within 50 m of the view and 100 m of
//!   the listener (0x5F1170) runs 0x4797B0: the object's first update
//!   (vtable +4, 0x46A1F0, once: +0x18) reads the record (the summed |J|
//!   +0x60, the point, the push's direction +0x70 = J / |J|), the kind
//!   (vtable +0x28) and the volume (vtable +0xC); then the play test
//!   (vtable +0x14) queues it (0x479770) or marks it done (vtable +0x1C,
//!   0x46A190: +0x20). Classes 0 and 1 do not play once done, so a record
//!   sounds at most once, at its first push (the clone holds props to the
//!   same; inferred for class 2).
//! * Volume (0x479A20, 0x4599E0): t = |J| / (3 · scale[kind]) (the curve
//!   0x82BD24 / 0x82BD5C: 0..3 → 0..1; scale 0x7884F4), 0 when t ≤ 0, at
//!   most 1; the byte is trunc(127 · t) (0x78B104). Class 0 (0x47A700)
//!   adds 50 (at most 127) when the car's up z (+0xB8) is under 0.9
//!   (0x7ED564) and fewer than all four wheels touch (0x5EC200: 0.25 a
//!   wheel flag +0x390..+0x393, under 1.0). Props (0x47A550) use |J| × the
//!   prop's 1/m (vtable +0x5C).
//! * The play test: class 0 (0x46ABA0) drops the sound while another live
//!   object of the same car with kind 5 (along a wall) has started and its
//!   first push ran within 0.8 (dot) of this record's latest push
//!   direction — the repeat on the same wall. (Its other test — the last
//!   started sound's direction 0x82BCB0 within 0.95, its point 0x82BCC0
//!   within 1 m, a floor normal, and 0.95·|n| < |n′| < 1.05·0.95·|n| — can
//!   never pass for the unit directions 0x5A9630 stores.) Class 1
//!   (0x459880) always plays. Class 2 (0x46A670) needs record flag bit 0
//!   (0x459600 → 0x592D10: a car in the pair with a sound object, car
//!   +0x130 → +0x18) or volume ≥ 10, and no other started sound of the same
//!   prop (0x592D90).
//! * The sample (vtable +0x4C: 0x47A770 track, 0x47A610 car, 0x479B40
//!   prop): the volume's level x = 0x7ECB78[clamp(trunc(v · 0.15625), 0,
//!   19)] and a counter per kind (or per prop material group) the member.
//!   Cars and track: trunc(4x) picks a group of 4 (kinds 2 and 6: 2), see
//!   [`index`]; class 0's level uses the volume + 60 (at most 127) when
//!   car +0x118 → +0x18 → +0x58 is 6: the car is a player's whose camera
//!   event kind is 6, the crash camera (`Impact::crash_camera`).
//!   Props by material (+0x140 = info +0xD8, 0x46A710): kind 7 (a car in
//!   the pair, 0x4598B0) base + 3·trunc(2x) + n mod 3, kind 8 base + 6 +
//!   3·trunc(3x) + n mod 3 ([`prop_index`]).
//! * The index is a stitch record (0x43BE60 on the manager's +0xD0 list
//!   +4, node +8): the list is built from `GLOBAL/InGameCommon.bun`'s
//!   `0x8003B500` chunks by 0x47BC30, each `0x3B502` record (hash, +4 gain,
//!   +6 its index, +8 the list: 0 collision, 1 the whooshes at +0xC, +9 the
//!   layer count) appended in file order (0x479130), its 16-byte layers in
//!   the `0x3B503` chunk after it (+0x10). List 0 has 289 records
//!   (0..0x120): the car and track kinds use 6..93, the props 94..288.
//! * Starting (0x46A540 → 0x479900 → 0x46CDD0): a voice from the manager's
//!   +0xBC pool (none: the object is done); the record's volume ×
//!   0x7F34A0[+6] >> 15; each layer (0x45B280 → 0x458360, its sound byte
//!   +0 into `IG_GLOBAL/MB_Stich_Collision.abk`'s 94-entry sound list, byte
//!   +1 the bank kind, 0 for all) at volume V × (+2 − rand(+0xA)) >> 15 ×
//!   record +4 >> 15, pitch (+4 − rand(|+0xC|)) × (0x1000 − p) / 0x1000
//!   (p = 0 at the start; 0x1000 = 1, at most 0x2000), azimuth +8 plus
//!   the object's, and +6 (at most 4000) as the start delay. rand(n) is
//!   0x43C1C0 (the seed 0x7EA624 mod n, then xorshift).
//!
// prov: inferred audio.collision.module
//! The start volume V (+0xC0) is the object's mix copy's level of its
//! kind's slot, rewritten each frame (0x461CE0), and the playing layers
//! follow it (0x47BB30 → 0x46CF70): **verified** and read under Wine
//! (docs/prop-hits.md: 0 on the start frame, then 12999 near a
//! barrel hit, falling as the car leaves; test
//! `impact_level_follows_the_original_per_frame`). **Inferred**: +6 in
//! milliseconds, the record's sound object (and its copy) living while
// prov: open audio.collision.module-2
//! its record or its layers do. **Open**: what fills the argument's +0xC
//! (the azimuth is the layer's word +8 plus it, 0x45B373 to 0x45B38E), the
//! pan law (the mixer is mono), the listener's attenuation. The caps:
//! at most 10 play requests queued per update and 5 objects playing, the
//! nearest kept ([`Pool`], 0x481850 / 0x7ECBC8). A water drum's splash (0x46AE00, material 5): see
//! `roadwind::EnvSfx::splash` (sound round 12) and [`splash_level`]: it
//! rides on the object's start and its level on the object's V.

use crate::audio::abk::Bank;
use crate::audio::fe::Sample;
use crate::formats::bundle::{chunks, read_i16, read_u16, read_u32, walk};
use glam::Vec3;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

/// The volume curve's scale by kind (0x7884F4, kinds 0–8).
pub const SCALE: [f32; 9] = [18.0, 2.5, 15.0, 1.0, 20.0, 8.0, 1.0, 2.5, 2.5];

/// The level curve (0x7ECB78).
static LEVELS: crate::exe_tables::Table<[f32; 20]> = crate::exe_tables::Table::new(|t| &t.collision_levels);

/// The per-record gain of list 0 (0x7F34A0, by the record's +6).
pub(crate) static GAIN: crate::exe_tables::Table<[u16; 289]> = crate::exe_tables::Table::new(|t| &t.collision_gain);

/// The round-robin counters per kind (0x82CBD8, 0x82CBDC, 0x82CBE8,
/// 0x82CBEC, 0x82CBE0, 0x82CBE4).
static COUNTERS: [AtomicU32; 6] = [const { AtomicU32::new(0) }; 6];

/// The props' counters: kind 7 (0x82CBA4..0x82CBD4) and kind 8
/// (0x82CB70..0x82CBA0), one per material group.
static PROP_COUNTERS: [[AtomicU32; 13]; 2] = [const { [const { AtomicU32::new(0) }; 13] }; 2];

/// By material (0 = outside 1..18): kind 7's base index and the counter
/// group (0x47A4BC / 0x47A504 jump tables; kind 8's base is 6 more).
/// SHOPPING_CART shares the default's counter, the other 259s their own.
const PROP: [(u32, usize); 19] = [
    (259, 0),
    (199, 1),
    (259, 0),
    (124, 2),
    (139, 3),
    (94, 4),
    (94, 4),
    (259, 5),
    (259, 5),
    (229, 6),
    (214, 7),
    (229, 6),
    (244, 8),
    (259, 5),
    (259, 5),
    (154, 9),
    (274, 10),
    (169, 11),
    (109, 12),
];

thread_local! {
    /// The state the audio's draws step: the game's shared generator
    /// (0x7EA624; Rand_Int 0x43C1C0 / Rand_Float 0x43C1F0) while the game's
    /// tick sounds run ([`enter_game_rng`] … [`leave_game_rng`]: the native
    /// audio objects draw on it, docs/rng.md, rows R7 / R13 /
    /// R30 of docs/native-fixes/N-RNG.tsv), else the audio's own copy (0x12345678, the draws made outside
    /// that window: **open**). Per thread, so parallel tests keep theirs.
    static SEED: std::cell::Cell<u32> = const { std::cell::Cell::new(0x1234_5678) };
}

/// The audio draws on the game's generator from now on: `game` is its
/// state (`GameRng.0`); returns the audio's own state, for
/// [`leave_game_rng`]. Between the two the game must not draw on its own
/// copy (possible once the audio's events are a function of the
/// ticks alone).
pub fn enter_game_rng(game: u32) -> u32 {
    SEED.with(|s| s.replace(game))
}

/// The end of [`enter_game_rng`]'s window: the audio's own state back
/// (`saved`); returns the game generator's state after the audio's draws.
pub fn leave_game_rng(saved: u32) -> u32 {
    SEED.with(|s| s.replace(saved))
}

/// A draw site that takes a `GameRng` (EA Trax's pick, 0x46B6F0) on the
/// audio's current generator ([`enter_game_rng`]).
pub(crate) fn with_rng<R>(f: impl FnOnce(&mut crate::opponents::GameRng) -> R) -> R {
    let mut g = crate::opponents::GameRng(SEED.with(|s| s.get()));
    let r = f(&mut g);
    SEED.with(|s| s.set(g.0));
    r
}

pub(crate) fn rand(n: u32) -> u32 {
    if n == 0 {
        return 0;
    }
    let s = SEED.with(|s| s.get());
    let r = s % n;
    let a = s ^ 0x1D87_2B41;
    let c = (a >> 5) ^ a;
    SEED.with(|x| x.set((c << 27) ^ c ^ a));
    r
}

/// The game's Rand_Float (0x43C1F0): (seed mod 0x7FFFFFFF) · 2^−31 · n,
/// the seed stepped as [`rand`]'s.
pub(crate) fn rand_float(n: f32) -> f32 {
    let s = SEED.with(|s| s.get());
    let r = s % 0x7FFF_FFFF;
    let a = s ^ 0x1D87_2B41;
    let c = (a >> 5) ^ a;
    SEED.with(|x| x.set((c << 27) ^ c ^ a));
    r as f32 * f32::from_bits(0x3000_0000) * n
}

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

/// The volume's level on the curve (0x7ECB78).
fn curve(volume: u8) -> f32 {
    LEVELS[((volume as f32 * 0.15625) as i32).clamp(0, 19) as usize]
}

/// The volume's level (0–3).
fn level(volume: u8) -> u32 {
    (curve(volume) * 4.0) as u32
}

fn next(counter: usize, members: u32) -> u32 {
    COUNTERS[counter].fetch_add(1, Ordering::Relaxed) % members
}

/// The stitch index a kind plays at a volume (0x47A610 / 0x47A770),
/// advancing that kind's counter (kind 6 shares kind 2's; another kind
/// plays 38).
pub fn index(kind: usize, volume: u8) -> usize {
    let l = level(volume);
    (match kind {
        0 => 6 + 4 * l + next(0, 4),
        1 => 22 + 4 * l + next(1, 4),
        2 | 6 => 70 + 2 * l + next(2, 2),
        3 => 78 + 4 * l + next(3, 4),
        4 => 38 + 4 * l + next(4, 4),
        5 => 54 + 4 * l + next(5, 4),
        _ => 38,
    }) as usize
}

/// A prop's stitch index (0x479B40) by kind (7 or 8) and material,
/// advancing its group's counter; another kind plays 0.
pub fn prop_index(kind: usize, material: usize, volume: u8) -> usize {
    let (base, group) = PROP[if (1..=18).contains(&material) { material } else { 0 }];
    let x = curve(volume);
    let (k, base, levels) = match kind {
        7 => (0, base, 2.0),
        8 => (1, base + 6, 3.0),
        _ => return 0,
    };
    let n = PROP_COUNTERS[k][group].fetch_add(1, Ordering::Relaxed) % 3;
    (base + 3 * (x * levels) as u32 + n) as usize
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
    /// object; see [`splash_level`]).
    pub splash: bool,
}

/// SFXObj_Collision's field update (0x461B60) on an impact object's first
/// mix pass (while the client's +0xBC is set, before 0x461CE0 clears it):
/// f0..f2 are cleared every pass; for the object's kind (+0x124, table
/// 0x461CC8: kinds 0 / 1 → f0 = 1 and f5, kinds 4 / 5 → f1 = 1 and f6,
/// kinds 7 / 8 → f2 = 1, else none) the level trunc(v · r) · 255 >> 15
/// (v the volume byte +0x50, r = trunc((1 − min(d, 40) / 40) · 32767) with
/// d the distance from the nearest player car (0x459A50) to the impact
/// point +0xF0, 0x43CE10) × trunc(effects · master · 32767) >> 15. The mix
/// map's section 5 runs an event control on each (trigger f0 scaled by
/// f6: −750 mB; trigger f1 scaled by f5: −720 mB; attack 0, hold 5 ×
/// 16.67 ms, release 11 × 16.67 ms) that sections 0 and 1 (the music,
/// speech and the player's sounds) read: a hard impact ducks them. Returns
/// (key = f, value) pairs for `mixmap::Scene::sfx`.
pub fn mix_fields(impact: &Impact, car: Vec3, effects_master: f32) -> Vec<(u32, i32)> {
    let d = impact.at.distance(car).max(0.0).min(40.0);
    let r = ((1.0 - d / 40.0) * 32767.0) as i32;
    let lv = ((i32::from(impact.volume) * r) * 255) >> 15;
    let v = (((effects_master * 32767.0) as i32) * lv) >> 15;
    match [0, 0, 3, 3, 1, 1, 3, 2, 2].get(impact.kind).copied().unwrap_or(3) {
        0 => vec![(0, 1), (5, v)],
        1 => vec![(1, 1), (6, v)],
        2 => vec![(2, 1)],
        _ => Vec::new(),
    }
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

/// A water drum's splash level (0x481FD0, each frame for each of its three
/// FX_ROADNOISE_TRANS requests): p1 = clamp(+0xC0 · [0x7ECF90] >> 15, 0,
/// 0x7FFF) with +0xC0 the prop object's start volume — the same field the
/// impact's layers take, written by its mix-map client SFXObj_Collision
/// (0x461CE0 at 0x461D85: section 5's level of the object's kind slot, 7
/// or 8, at its point; **verified**: 0x482015 reads `[edi+0xC0]` on the
/// object whose vtable 0x78BB08 holds 0x46AE00 at slot 8 and 0x481FD0 at
/// slot 9). The clone once scaled the impact's volume byte to
/// 0x7FFF instead (a 127 hit: 17000 against the original's 6744 at 1 m,
/// +8 dB). The ramp by +0x11C when +0x128 has bit 0x10 is not applied
/// (**open**: that flag's writer).
pub fn splash_level(start: i32) -> i32 {
    (start * crate::audio::world::env::SPLASH_SCALE >> 15).clamp(0, 0x7FFF)
}

/// One layer of a stitch record (16 bytes).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layer {
    /// +0: the entry in the bank's sound list; +1: the bank kind.
    pub sound: u8,
    pub bank: u8,
    /// +2 / +0xA: volume (0x3FFF typical) less rand(+0xA).
    pub volume: u16,
    pub volume_rand: u16,
    /// +4 / +0xC: pitch (0x1000 = 1) less rand(|+0xC|).
    pub pitch: u16,
    pub pitch_rand: i16,
    /// +6: the start delay (at most 4000; inferred ms).
    pub delay: u16,
    /// +8: azimuth added to the object's (0x10000 a turn).
    pub azimuth: u16,
}

/// A stitch record (`0x3B502`) and its layers (`0x3B503`).
#[derive(Clone, Debug, PartialEq)]
pub struct Stitch {
    pub hash: u32,
    /// +4: the record's gain (0x7FFF for all).
    pub gain: u16,
    /// +6: its index (the gain table's).
    pub index: u16,
    pub layers: Vec<Layer>,
}

/// The stitch lists (0x47BC30): records by the list byte (+8), in file
/// order.
pub fn parse_stitches(data: &[u8]) -> [Vec<Stitch>; 2] {
    let mut lists: [Vec<Stitch>; 2] = Default::default();
    walk(chunks(data, 0), &mut |c| {
        if c.id != 0x8003_B500 {
            return;
        }
        let mut kids = c.children().peekable();
        while let Some(head) = kids.next() {
            if head.id != 0x3_B502 {
                break;
            }
            let h = head.body;
            let layers_chunk = kids.next();
            if h.len() < 10 {
                continue;
            }
            let body = layers_chunk.map(|l| l.body).unwrap_or(&[]);
            let layers = (0..h[9] as usize)
                .filter(|i| (i + 1) * 16 <= body.len())
                .map(|i| {
                    let e = &body[i * 16..];
                    Layer {
                        sound: e[0],
                        bank: e[1],
                        volume: read_u16(e, 2),
                        pitch: read_u16(e, 4),
                        delay: read_u16(e, 6),
                        azimuth: read_u16(e, 8),
                        volume_rand: read_u16(e, 0xA),
                        pitch_rand: read_i16(e, 0xC),
                    }
                })
                .collect();
            if let Some(list) = lists.get_mut(h[8] as usize) {
                list.push(Stitch { hash: read_u32(h, 0), gain: read_u16(h, 4), index: read_u16(h, 6), layers });
            }
        }
    });
    lists
}

/// One layer as the mixer takes it: the sample, gain (0–1), pitch (0x1000
/// = 1) and delay in seconds.
pub type Voice = (Sample, f32, u16, f32);

/// The layers a stitch record plays at a start volume V (0..0x7FFF;
/// 0x46CDD0 → 0x45B280: V × the record list's gain 0x7F34A0[+6] >> 15 ×
/// the record's +4 >> 15, then each layer's own volume).
///
/// V is the sound object's +0xC0 (the start block +0xBC's second dword,
/// copied by 0x479900 → 0x46CDD0 and again every frame by 0x47BB30 →
/// 0x46CF70, which sets each playing layer to its volume × V >> 15), and
/// +0xC0 is written each frame by the object's mix-map client
/// SFXObj_Collision (vtable 0x78B078, slot 0x28 = 0x461CE0 at 0x461D85):
/// slot 0x38 (0x45E390) of the object's kind (+0x124: 1..8 → slot 1..8,
/// else 0) — the collision section's level for that slot at the object's
/// point (**verified**). The impact's volume byte picks the sample
/// only. Before this fix the clone took V from the byte.
pub fn voices(
    stitch: &Stitch,
    list_index: usize,
    start: i32,
    mut sample: impl FnMut(usize) -> Option<Sample>,
) -> Vec<Voice> {
    let v = start.clamp(0, 0x7FFF) as i64;
    let v = v * *GAIN.get(list_index).unwrap_or(&0x7FFF) as i64 >> 15;
    let v = v * stitch.gain as i64 >> 15;
    stitch
        .layers
        .iter()
        .filter_map(|l| {
            let layer_volume = l.volume as i64 - rand(l.volume_rand as u32) as i64;
            let gain = (v * layer_volume >> 15).clamp(0, 0x7FFF) as f32 / 32767.0;
            let pitch = (l.pitch as i64 - rand(l.pitch_rand.unsigned_abs() as u32) as i64).clamp(0, 0x2000) as u16;
            let delay = l.delay.min(4000) as f32 / 1000.0;
            Some((sample(l.sound as usize)?, gain, pitch, delay))
        })
        .collect()
}

/// The collision manager's play queue and its playing objects (0x481850
/// at each collision-manager update 0x481BD0): an impact object is queued
/// at its first push (0x479770; none while 10 or more were queued this
/// update, 0x481CD0's `+0xC > 9`); the flush starts every queued one when
/// queued + playing ≤ 5 (0x7ECBC8), else keeps the 5 nearest (priority
/// +0xA8 = 0x5F1170: the distance to the listener) and stops the farther
/// playing ones (vtable +0x48, 0x82B950−−). An object plays while its
/// layers do. **Not modelled**: record flag bit 0 (+0x128, a car with a
/// sound object in the pair), which protects an object from the steal.
#[derive(Default)]
pub struct Pool {
    /// The playing objects: their layers' voice handles and distance.
    playing: Vec<(Vec<u32>, f32)>,
}

/// The playing collision objects' cap (0x7ECBC8).
pub const PLAYING_CAP: usize = 5;
/// The play requests one update may queue (0x481CD0: `9 < +0xC`).
pub const QUEUE_CAP: usize = 10;

impl Pool {
    /// One flush: `queued` = (distance, layers) of this update's new
    /// objects (at most [`QUEUE_CAP`] are kept); `alive(id)` tells a
    /// voice still plays, `stop(id)` stops one, `start` starts a layer and
    /// returns its handle. Returns the objects started.
    pub fn flush(
        &mut self,
        queued: Vec<(f32, Vec<Voice>)>,
        alive: &mut dyn FnMut(u32) -> bool,
        stop: &mut dyn FnMut(u32),
        start: &mut dyn FnMut(&Voice) -> u32,
    ) -> usize {
        self.flush_started(queued, alive, stop, start).len()
    }

    /// [`Pool::flush`], returning which of `queued` (by index) were started
    /// (the flush's start call, vtable +0x20 at 0x4818A6: for a prop object
    /// 0x46AE00, which also starts a water drum's splash).
    pub fn flush_started(
        &mut self,
        queued: Vec<(f32, Vec<Voice>)>,
        alive: &mut dyn FnMut(u32) -> bool,
        stop: &mut dyn FnMut(u32),
        start: &mut dyn FnMut(&Voice) -> u32,
    ) -> Vec<usize> {
        self.playing.retain(|p| p.0.iter().any(|&id| alive(id)));
        let mut queued: Vec<(usize, (f32, Vec<Voice>))> = queued.into_iter().enumerate().collect();
        queued.truncate(QUEUE_CAP);
        if self.playing.len() + queued.len() > PLAYING_CAP {
            // The nearest five of both lists stay (queued first on a tie).
            let mut all: Vec<(f32, bool, usize)> = queued
                .iter()
                .enumerate()
                .map(|(i, q)| (q.1.0, false, i))
                .chain(self.playing.iter().enumerate().map(|(i, p)| (p.1, true, i)))
                .collect();
            all.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            let keep: Vec<(bool, usize)> = all.iter().take(PLAYING_CAP).map(|e| (e.1, e.2)).collect();
            let mut k = 0;
            self.playing.retain(|p| {
                let i = k;
                k += 1;
                let kept = keep.contains(&(true, i));
                if !kept {
                    p.0.iter().for_each(|&id| stop(id));
                }
                kept
            });
            let mut k = 0;
            queued.retain(|_| {
                let i = k;
                k += 1;
                keep.contains(&(false, i))
            });
        }
        let mut started = Vec::with_capacity(queued.len());
        for (k, (d, layers)) in queued {
            let ids = layers.iter().map(|v| start(v)).collect();
            self.playing.push((ids, d));
            started.push(k);
        }
        started
    }

    /// The objects playing (a check aid).
    pub fn playing(&self) -> usize {
        self.playing.len()
    }
}

/// The collision bank (`SOUND/IG_GLOBAL/MB_Stich_Collision.abk`), the
/// stitch records and the samples decoded so far.
pub struct CollisionSounds {
    bank: Arc<Bank>,
    list: Vec<u32>,
    stitches: Vec<Stitch>,
    cache: HashMap<usize, Option<Sample>>,
}

impl CollisionSounds {
    pub fn load(root: &Path) -> Option<CollisionSounds> {
        // The bank and the stitch lists the stitch banks share.
        let bank = crate::audio::banks::cache::shared_bank(root, "SOUND/IG_GLOBAL/MB_Stich_Collision.abk")?;
        let list =
            bank.modules.first().map(|m| bank.sound_list(m).into_iter().map(|e| e.1).collect()).unwrap_or_default();
        let stitches = crate::audio::banks::cache::shared_stitches(root)?[0].clone();
        Some(CollisionSounds { bank, list, stitches, cache: HashMap::new() })
    }

    /// The sample at a sound-list entry.
    pub fn sample(&mut self, index: usize) -> Option<Sample> {
        if let Some(s) = self.cache.get(&index) {
            return s.clone();
        }
        let s = self
            .list
            .get(index)
            .and_then(|&slot| self.bank.decode(slot as usize))
            .map(|(info, pcm)| Arc::new((info.rate, pcm)));
        self.cache.insert(index, s.clone());
        s
    }

    /// The layers an impact plays at its start volume (the mix level of
    /// its kind's slot, see [`voices`]; none below volume byte 1).
    pub fn play(&mut self, impact: &Impact, start: i32) -> Vec<Voice> {
        if impact.volume == 0 {
            return Vec::new();
        }
        let n = if impact.kind >= 7 {
            prop_index(impact.kind, impact.material, impact.volume)
        } else {
            // 0x47A770 (the track kinds 2–6) takes the volume + 60, at most
            // 127, for its level under a player's crash camera; the
            // layers' own volume stays the impact's.
            let v = if impact.crash_camera && (2..=6).contains(&impact.kind) {
                impact.volume.saturating_add(60).min(127)
            } else {
                impact.volume
            };
            index(impact.kind, v)
        };
        let Some(stitch) = self.stitches.get(n).cloned() else { return Vec::new() };
        voices(&stitch, n, start, |i| self.sample(i))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_most_five_objects_play_the_nearest() {
        let s: Sample = std::sync::Arc::new((22050, vec![0.0; 10]));
        let layer = (s, 1.0f32, 0x1000u16, 0.0f32);
        let mut pool = Pool::default();
        let mut live: Vec<u32> = Vec::new();
        let mut next = 0;
        let q = |ds: &[f32]| ds.iter().map(|&d| (d, vec![layer.clone()])).collect::<Vec<_>>();
        // Twelve at once: ten queue (0x481CD0), five play — the nearest.
        let started = {
            let mut alive = |id: u32| live.contains(&id);
            let mut stop = |_: u32| {};
            let mut started = Vec::new();
            let mut start = |_: &Voice| {
                next += 1;
                started.push(next);
                next
            };
            let n = pool.flush(
                q(&[9.0, 8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0, 0.5, 0.1, 0.2]),
                &mut alive,
                &mut stop,
                &mut start,
            );
            assert_eq!(n, PLAYING_CAP);
            started
        };
        live.extend(started);
        assert_eq!(pool.playing(), 5);
        // A nearer one steals the farthest playing one.
        let mut stopped = Vec::new();
        let n = pool.flush(q(&[0.0]), &mut |id| live.contains(&id), &mut |id| stopped.push(id), &mut |_| 100);
        assert_eq!((n, stopped.len(), pool.playing()), (1, 1, 5));
        // A farther one waits for none: it is dropped.
        let n = pool.flush(q(&[50.0]), &mut |id| live.contains(&id) || id == 100, &mut |_| {}, &mut |_| 101);
        assert_eq!(n, 0);
    }

    /// A water drum's splash level is its object's start volume
    /// +0xC0 × [0x7ECF90] (17000) >> 15 (0x482015..0x482076), clamped to
    /// 0..0x7FFF — not the impact's byte scaled to 0x7FFF. At 1 m a prop's
    /// copy gives slot 7 −800 mB = 12999 (`mixmap::tests::impact_slots_by_kind`):
    /// 6743, where the old law gave 17000 for a 127 hit (+8.0 dB).
    #[test]
    fn splash_level_follows_the_objects_start_volume() {
        assert_eq!(splash_level(12999), 6743);
        assert_eq!(splash_level(0x7FFF), 16999);
        assert_eq!(splash_level(0), 0);
        assert_eq!(splash_level(-5), 0);
        let before = (127 * 0x7FFF / 0x7F) * crate::audio::world::env::SPLASH_SCALE >> 15;
        let db = 20.0 * (before as f32 / splash_level(12999) as f32).log10();
        assert!((db - 8.0).abs() < 0.1, "{db}");
    }

    /// The flush reports which queued objects it started (the start
    /// call 0x4818A6 that begins a drum's splash), so a splash is capped
    /// with its object: of seven at once only the five nearest start.
    #[test]
    fn flush_reports_the_started_objects() {
        let s: Sample = std::sync::Arc::new((22050, vec![0.0; 10]));
        let layer = (s, 1.0f32, 0x1000u16, 0.0f32);
        let mut pool = Pool::default();
        let q: Vec<(f32, Vec<Voice>)> =
            [7.0, 1.0, 6.0, 2.0, 5.0, 3.0, 4.0].iter().map(|&d| (d, vec![layer.clone()])).collect();
        let mut n = 0;
        let mut started = pool.flush_started(q, &mut |_| true, &mut |_| {}, &mut |_| {
            n += 1;
            n
        });
        started.sort_unstable();
        assert_eq!(started, vec![1, 3, 4, 5, 6]);
    }

    /// Against the original: two barrel hits' collision sound
    /// objects read under Wine every frame (`docs/evidence/prop-hits/
    /// native-impact-levels.tsv`, tools/parity/proppoll.py: object +0xC0 V,
    /// +0xF0 the point, the listener globals 0x82BEDC car / 0x82BF10
    /// camera). V is 0 on the start frame (0x46A540 zeroes +0xC0 before
    /// 0x479900), then each frame the copy's level of slot 7 at its point
    /// (0x461CE0) — 12999 near, falling as the car drives away. The clone's
    /// mix map, given each sample's geometry, gives the same level (median
    /// 0.12 dB off): what the playing layers and a drum's splash follow each
    /// frame (`Game::mix_levels`).
    #[test]
    #[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
    fn impact_level_follows_the_original_per_frame() {
        use crate::audio::mixmap::{Listener, Mix, MixMap, Scene, master_vol_fields};
        let map = MixMap::load(&crate::testkit::game_root(), 0).expect("loading the mix map");
        let rows = &crate::testkit::private_str("docs/evidence/prop-hits/native-impact-levels.tsv");
        let xy = |s: &str| -> [f32; 2] {
            let v: Vec<f32> = s.split(',').map(|x| x.parse().unwrap()).collect();
            [v[0], v[1]]
        };
        let mut db: Vec<f32> = Vec::new();
        let mut falling = false;
        for l in rows.lines().filter(|l| !l.starts_with(['#', 'c'])) {
            let c: Vec<&str> = l.split('\t').collect();
            let native: i32 = c[5].parse().unwrap();
            if native == 0 {
                continue;
            }
            let mut mix = Mix::new(map.clone());
            let scene = Scene {
                listener: Some(Listener { cam: xy(c[7]), cam_dir: [0.0, 1.0], car: xy(c[6]), car_dir: [0.0, 1.0] }),
                master_vol: Some(master_vol_fields(1.0, [0.7, 1.0, 1.0, 1.0, 1.0, 1.0], false)),
                collisions: vec![xy(c[3])],
                ..Default::default()
            };
            mix.frame(&scene, 1.0 / 60.0);
            let clone = mix.level(5, 0, 0, 7);
            db.push((20.0 * (clone as f32 / native as f32).log10()).abs());
            falling |= native < 3000;
        }
        db.sort_by(f32::total_cmp);
        let (n, median, p90) = (db.len(), db[db.len() / 2], db[db.len() * 9 / 10]);
        assert!(n > 100 && falling, "{n} samples");
        // Measured: median 0.12 dB, 90th percentile 1.02 dB; the rest are
        // the frames where the original's level steps a frame before the
        // polled listener shows it (up to 2.9 dB), and beyond ~15 m the
        // clone's level runs ~1 dB high (**open**, docs/prop-hits.md).
        assert!(median <= 0.25 && p90 <= 1.25, "median {median} dB, p90 {p90} dB over {n} samples");
    }

    #[test]
    fn impact_fields_follow_the_kind_distance_and_volumes() {
        let i = Impact { at: Vec3::new(10.0, 0.0, 0.0), kind: 0, volume: 127, material: 0, crash_camera: false, splash: false };
        // d = 10 of 40: r = 24575; 127 · r · 255 >> 15 = 24287, × 32767 >> 15.
        assert_eq!(mix_fields(&i, Vec3::ZERO, 1.0), vec![(0, 1), (5, 24286)]);
        assert_eq!(mix_fields(&Impact { kind: 4, ..i }, Vec3::ZERO, 0.5), vec![(1, 1), (6, 12142)]);
        assert_eq!(mix_fields(&Impact { kind: 7, ..i }, Vec3::ZERO, 1.0), vec![(2, 1)]);
        assert!(mix_fields(&Impact { kind: 2, ..i }, Vec3::ZERO, 1.0).is_empty());
        assert_eq!(mix_fields(&Impact { at: Vec3::new(50.0, 0.0, 0.0), ..i }, Vec3::ZERO, 1.0), vec![(0, 1), (5, 0)]);
    }

    #[test]
    fn volume_follows_the_curve() {
        assert_eq!(volume(0.0, 0), 0);
        assert_eq!(volume(27.0, 0), 63);
        assert_eq!(volume(100.0, 1), 127);
        assert_eq!(volume(3.75, 1), 63);
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn kinds_tile_the_bank() {
        assert_eq!(level(0), 0);
        assert_eq!(level(127), 3);
        for kind in 0..6 {
            for v in [0u8, 40, 127] {
                let i = index(kind, v);
                assert!((6..94).contains(&i), "{kind} {v} {i}");
            }
        }
        assert_eq!(track_kind(Vec3::X, Vec3::Z, Vec3::X), 4);
        assert_eq!(track_kind(Vec3::X, Vec3::Z, Vec3::Y), 5);
        assert_eq!(track_kind(Vec3::X, Vec3::Z, Vec3::Z), 3);
        assert_eq!(track_kind(Vec3::X, -Vec3::Z, Vec3::Z), 2);
        assert_eq!(car_kind(Vec3::X, Vec3::ZERO, Vec3::X, Vec3::Y * 2.0, Vec3::Y), 1);
        assert_eq!(car_kind(Vec3::X, Vec3::ZERO, Vec3::Y, Vec3::X * 4.0, Vec3::X * 2.0), 0);
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn prop_indices_cover_the_list() {
        for m in 0..19 {
            for v in [0u8, 40, 127] {
                let (a, b) = (prop_index(7, m, v), prop_index(8, m, v));
                assert!((94..289).contains(&a) && (94..289).contains(&b), "{m} {v} {a} {b}");
            }
        }
        assert_eq!(prop_index(7, 5, 0) / 3, 94 / 3);
        assert!(prop_index(8, 16, 127) <= 288);
        assert_eq!(prop_index(3, 1, 0), 0);
        assert_eq!(tipped(100, 0.5, 3), 127);
        assert_eq!(tipped(10, 0.5, 4), 10);
        assert_eq!(tipped(10, 0.95, 0), 10);
        assert!(repeats_wall(Vec3::X, [(5, Vec3::X)]));
        assert!(!repeats_wall(Vec3::X, [(4, Vec3::X), (5, Vec3::Y)]));
        assert!(prop_plays(7, 0, false) && !prop_plays(8, 9, false) && !prop_plays(7, 50, true));
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn stitch_lists_load() {
        let data = std::fs::read(crate::testkit::game_root().join("GLOBAL/InGameCommon.bun")).expect("GLOBAL/InGameCommon.bun");
        let [collision, whoosh] = parse_stitches(&data);
        assert_eq!(collision.len(), 289);
        assert_eq!(whoosh.len(), 60);
        for (i, s) in collision.iter().enumerate() {
            assert_eq!(s.index as usize, i);
            assert_eq!(s.gain, 0x7FFF);
            assert!(!s.layers.is_empty());
            assert!(s.layers.iter().all(|l| l.sound < 94 && l.bank == 0 && l.delay <= 4000));
        }
        assert_eq!(
            collision[0].layers[1],
            Layer {
                sound: 34,
                bank: 0,
                volume: 16383,
                volume_rand: 4800,
                pitch: 4096,
                pitch_rand: 0,
                delay: 957,
                azimuth: 0
            }
        );
        let v = voices(&collision[0], 0, 0x7FFF, |_| Some(Arc::new((36000, vec![0.0; 4]))));
        assert_eq!(v.len(), 3);
        assert!(v.iter().all(|x| x.1 > 0.0 && x.1 <= 0.75 && x.2 <= 0x2000));
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn bank_holds_every_index() {
        let mut c = CollisionSounds::load(&crate::testkit::game_root()).expect("loading the collision sounds");
        assert_eq!(c.list.len(), 94);
        for i in [6, 37, 38, 93] {
            let s = c.sample(i).unwrap();
            assert_eq!(s.0, 36000);
            assert!(s.1.iter().any(|x| x.abs() > 100.0));
        }
    }
}
