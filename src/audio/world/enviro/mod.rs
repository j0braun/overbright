//! The world's own sound objects: the planes, the trains and the fountains
//! (sound6; verified unless marked). Each is a sound state of its own
//! class (the state registry list 0x82BEC4: CSTATE_Fountain 0x40000,
//! CSTATE_Plane 0x70000, CSTATE_Train 0x80000) holding a sound object
//! (SFXObj_Fountain / _Plane / _Train, ids 0x40000 / 0x70000 / 0x80000)
//! and its position controller (SFXCTL_3DFountainPos / 3DEnviroObjPos /
//! 3DTrainObjPos: the 3D car position's update 0x471DC0 and fields,
//! 0x45E100 → 0x45DF00, over a pointed-to position, slot +0x2C 0x45E1D0).
//! The mix map gives them sections 4, 7 and 8, class 0, one copy per state.
//!
//! * **Managers** (made by 0x485600.. with (class, the most): fountains
//!   (4, 5) vtable 0x78A550, planes (7, 12) 0x78A4E0, trains (8, 8)
//!   0x78A518). Their set-up (vtable +8) makes the states: the planes 3
//!   (0x476E00), the trains 6 (0x476EB0, the train table's states
//!   cleared), the fountains 5 only on tracks 4000..4199 (0x476F00,
//!   g_TrackNumber 0x89E7A0). A state's attach (+0x28, 0x464D30) sets
//!   +0x40 = the table entry, +0x3C = 1 and starts its objects (slot
//!   0x2C); release (+0x2C, 0x464D60) clears them and stops the objects
//!   (slot 0x30).
//! * **The update** (vtable +0x18, each frame with at least one player,
//!   0x8900BC): for each table entry in order, its position (planes and
//!   trains: 0x439F40, the world animation's entity; fountains: the
//!   table's point); d = its distance (3D, 0x43CE10) to the nearest
//!   player's car (0x459A50). A state already on the entry is released
//!   when d ≥ R or (planes, trains) the animation waits; else, with d < R
//!   and not waiting, a free state (+0x30) is attached; with none free,
//!   the first busy state whose own entry is farther from its nearest
//!   player than d is released and attached to this one. R: planes 2000
//!   m (0x464410), trains 400 m (0x464600), fountains 150 m (0x4647E0).
//!   A train entry whose animation 0x439F40 does not find is marked 2 and
//!   skipped from then on. Then the states' objects run (0x463BD0).
//! * **The tables.** Planes (0x476E00, by the race at the set-up): a drag
//!   (0x89E7D8 / 0x890108) on tracks 4400..4499 0x7ED718, another drag
//!   0x7ED6D8 (5 entries), a URL race (0x89E7E1) 0x7ED760 (3), else
//!   0x7ED690 (6); entries {instance number, entity hash}. Trains:
//!   0x7ED798, 24 entries. Fountains: 0x7F0D70 (7 points, variant 0) then
//!   0x7F0B80 (31, variant 1), the variant kept in the state's +0x44.
//!   In the shipped data the free-roam plane table finds all six (L4RA
//!   instances 0x22..0x2C), the train table only instance 0x6A's three
//!   entities (the other numbers name instances L4RA does not have, or
//!   their entities are under other numbers).
//! * **0x439F40**(number, hash): the world animation instance whose
//!   record +0x00 is the number (0x432170 on [0x827B94]), its entity
//!   controller whose hash is `hash`, that entity node's world position
//!   (+0x40, after 0x60BB20); "waiting" when the controller's player is
//!   in its start delay (0x431610) or its pause (0x431660).
//! * **SFXObj_Plane** (vtable 0x78B530; ctor 0x476090). Position (slot 9,
//!   0x462AE0, with the state on): +0xC0 = 0x439F40, +0xD0 = its 3D move
//!   since the last update. Start (slot 11, 0x462810): the position, the
//!   controller pointed at it (+0x2C) and given (0, 20000) (+0x28); in a
//!   URL race unless game mode 4 with race type 8 (0x457BA0): FX_MAIN_MEM
//!   (`FX_MAIN_MEM_MB`, slot 0x82C548) {0, 0 …} → +0xDC, else ENV_STATIC
//!   (`ENV_COMMON_MB`, 0x82C510) {1, 0 …} → +0xD8. Frame (slot 10,
//!   0x462B70): a move below 0.3 m (0x78758C) stops both; else with
//!   neither playing the same choice starts one. FX_MAIN_MEM: p1 =
//!   clamp(slot 0x38(1), 0, 0x7FFF), p2 = clamp(trunc(doppler(5) ·
//!   4096), 0, 0x4000), p3 = clamp(slot 0x3C(1), 0, 0xFFFF), p4 =
//!   clamp(trunc(min(field 0, 1000) · 2.6 (0x78A31C), negated when the
//!   azimuth is in (0x4000, 0xC000)), −1000, 1000) — field 0 the
//!   controller's distance (0x45DE50 reads +8); ENV_STATIC: p0 = 1, p1 =
//!   clamp(slot 0x38(0), 0, 0x7FFF), p2 = clamp(trunc(doppler(5) · 4096),
//!   0, 0x1FA0), p3 = clamp(slot 0x3C(0), 0, 0xFFFF). doppler(5) = depth
//!   0.3 (0x7F3774).
//! * **SFXObj_Train** (vtable 0x78B578). Start (0x463020): the position,
//!   the controller pointed at it, ENV_STATIC {0, 0 …} → +0xD8. Frame
//!   (0x4631E0): p0 = 0, p1 = clamp(slot 0x38(0), 0, 0x7FFF), p2 = 0x1000,
//!   p3 = clamp(slot 0x3C(0), 0, 0xFFFF). Stop (0x463180).
//! * **SFXObj_Fountain** (vtable 0x78B650; ctor 0x4766C0). Start
//!   (0x463920): the controller pointed at the table point, ENV_STATIC {4,
//!   0 …} → +0xB0. Frame (0x463820, with the state on): variant 0: p1 =
//!   clamp(slot 0x38(0), 0, 0x7FFF), p2 = 0x1450, p3 = clamp(slot
//!   0x3C(0), 0, 0xFFFF); variant 1: slot 1, p2 = 0xFA0. Stop (0x463A60).
//!
//! ENV_COMMON_MB's ENV_STATIC plays sound 1 for variant 0 (the train), 2
//! for 1 (the plane) and 3 for 4 (the fountain).
//!
// prov: inferred audio.enviro.module
//! **Inferred**: the objects' frame once per game frame after the
//! managers' update, the doppler's dt the frame time, the plane's 0.3 m
//! move test per 60 Hz frame (the clone's frames vary); game mode 4 never
//! (offline).

mod manager;
mod objects;
mod tables;

pub use tables::*;

use crate::audio::carfx::{CarFxSounds, EventBank};
use crate::audio::mixmap::Levels;
use crate::audio::traffic::Doppler;
use glam::Vec3;
use std::collections::HashMap;

/// One world-animation entity as 0x439F40 finds it.
#[derive(Clone, Copy, Debug)]
pub struct AnimPoint {
    /// The instance record's +0x00.
    pub id: u32,
    /// The entity's hash.
    pub hash: u32,
    pub position: Vec3,
    /// The entity's player waits (start delay or pause).
    pub hidden: bool,
}

/// The race kind the plane table is picked by (0x476E00).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Setup {
    pub track: i32,
    pub drag: bool,
    pub url: bool,
}

/// The plane table for a set-up.
pub fn plane_table(s: &Setup) -> &'static [(u32, u32)] {
    if s.drag {
        if (4400..4500).contains(&s.track) { &PLANES } else { &PLANES_DRAG }
    } else if s.url {
        &PLANES_URL
    } else {
        &PLANES
    }
}

#[derive(Clone, Debug, Default)]
struct State {
    /// +0x40: the table entry (None: free, +0x3C clear).
    entry: Option<usize>,
    /// +0x44 (fountains): the point's variant.
    variant: i32,
    /// The object's position (+0xC0) and the last (+0xB0), its move +0xD0.
    pos: Vec3,
    last: Vec3,
    moved: f32,
    doppler: Doppler,
    /// ENV_STATIC (+0xD8 / +0xB0) and FX_MAIN_MEM (+0xDC).
    env: Option<u32>,
    main: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Fountain,
    Plane,
    Train,
}

impl Kind {
    fn radius(self) -> f32 {
        match self {
            Kind::Fountain => 150.0,
            Kind::Plane => 2000.0,
            Kind::Train => 400.0,
        }
    }
}

/// One manager: its states and its table's entries.
#[derive(Clone, Debug)]
struct Pool {
    kind: Kind,
    states: Vec<State>,
    /// Train entries 0x439F40 did not find (state 2).
    dead: Vec<bool>,
}

impl Pool {
    fn new(kind: Kind, n: usize, entries: usize) -> Pool {
        Pool { kind, states: vec![State::default(); n], dead: vec![false; entries] }
    }
}

/// What the world gives the objects each frame.
pub struct Input<'a> {
    pub points: &'a [AnimPoint],
    /// The players' cars (the first is the listener's).
    pub players: &'a [Vec3],
    /// The listener's car position (the controllers' field 0 and the
    /// doppler are taken from it, as the position objects' fields).
    pub listener: Vec3,
    /// The mix map's copies of sections 4 (fountains), 7 (planes) and 8
    /// (trains), in state order.
    pub mix: [&'a [Levels]; 3],
    /// A URL race (0x89E7E1), for the plane's start choice.
    pub url: bool,
}

const ENV_COMMON: &str = "SOUND/IG_GLOBAL/ENV_COMMON_MB.abk";
const MAIN_MEM: &str = "SOUND/IG_GLOBAL/FX_MAIN_MEM_MB.abk";

pub struct Enviro {
    sounds: CarFxSounds,
    gains: HashMap<u32, (f32, f32, f32)>,
    env_bank: Option<EventBank>,
    main_bank: Option<EventBank>,
    setup: Option<Setup>,
    fountains: Pool,
    planes: Pool,
    trains: Pool,
    /// Events started this frame (for the self-test and the debug log).
    pub started: usize,
    /// What started this frame: (kind name, table entry, distance m, level).
    pub log: Vec<(&'static str, usize, f32, i32)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plane_tables_by_race() {
        assert_eq!(plane_table(&Setup { track: 4000, drag: false, url: false }).len(), 6);
        assert_eq!(plane_table(&Setup { track: 4401, drag: true, url: false }).len(), 6);
        assert_eq!(plane_table(&Setup { track: 4201, drag: true, url: false }).len(), 5);
        assert_eq!(plane_table(&Setup { track: 4101, drag: false, url: true }).len(), 3);
    }

    /// The six free-roam planes and train instance 0x6A's three entities
    /// are in L4RA's world animations; the other train entries are not.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn l4ra_finds_the_free_roam_planes() {
        let root = crate::testkit::game_root();
        let bun = std::fs::read(root.join("TRACKS/L4RA.BUN")).expect("TRACKS/L4RA.BUN");
        let wa = crate::formats::world_anim::WorldAnimations::parse(&bun);
        let has =
            |id: u32, hash: u32| wa.instances.iter().any(|i| i.id == id && wa.tree(i.tree).any(|e| e.hash == hash));
        assert!(PLANES.iter().all(|&(id, h)| has(id, h)));
        let trains: Vec<usize> = (0..TRAINS.len()).filter(|&k| has(TRAINS[k].0, TRAINS[k].1)).collect();
        assert_eq!(trains, vec![9, 10, 11]);
    }
}
