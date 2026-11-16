//! Car LOD selection by screen size.

use super::*;

/// The LODs (bit n: LOD n) the in-game car draw (0x620AF0) draws a slot's
/// model group at, besides the model being there: slots 1, 2, 9, 10 and 11
/// below LOD D (0x623384, 0x6233C9, 0x62340B), slot 9's group 1 at LOD A
/// only (0x6233BF); slots 0x13–0x16 and 0x1A at LOD A only (0x62327D,
/// 0x6232B5, 0x6232E5, 0x623349); the wheels (rims 0x1D / 0x1E, the
/// spinner their group 1) only while the LOD is at most C (0x62751A; the
// prov: open car.lod_gate
/// wheel model's LOD, esp+0xF4, clamped to 2 at 0x627650). **Open**: slots
/// 0x11 / 0x12 below D when esp+0xD8 is set (not traced). The rear plate
/// (slot 0x21, the REAR_LICENSE_PLATE block 0x624CBD…) is skipped past LOD
/// C (0x624CAB: LOD > 2 and the draw's argument +0x2C clear, which the
/// world pass 0x6316E0 always passes as 0).
pub fn lod_gate(slot: usize, group: usize) -> u8 {
    match slot {
        0x21 => 0b0111,
        9 if group == 1 => 0b0001,
        1 | 2 | 9 | 10 | 11 => 0b0111,
        0x13..=0x16 | 0x1A => 0b0001,
        0x1D | 0x1E => 0b0111,
        _ => 0b1111,
    }
}

/// Slot 8 (TOP, ride +0x5B0) in the generic part draw (0x6230D5; slot
/// switch 0x630D68 → 0x62315B): for the car types 0x60C860 accepts (3, 5,
/// 0x12, 0x17: 240SX, CIVIC, RSX, G35, whose `ROOF` has a sunroof) with a
/// roof scoop (ride +0x5AC, slot 7) above level 0 only group 1 is drawn
/// (`FULLROOF`, the closed roof under the scoop); otherwise only group 0
/// (`ROOF`). Other slots: both groups (whenever their solid exists).
pub fn top_group_drawn(slot: usize, group: usize, type_index: usize, scoop_level: u8) -> bool {
    if slot != 8 {
        return true;
    }
    let closed = matches!(type_index, 3 | 5 | 0x12 | 0x17) && scoop_level > 0;
    group == usize::from(closed)
}

/// 0x48ACB0(view, p, 3.0): a car's size on screen (pixels at the view's
/// scale k = view +0x0C = 256 / tan(fov / 2), 0x5C7ED0): 0 when p lies
/// more than r behind the eye (along its forward, +0x50); else
/// ftol(k / (d − r) · r) for d − r > r / 2 (0x784260), d the distance,
/// and ftol(2k) nearer.
pub fn screen_size(eye: Vec3, forward: Vec3, k: f32, p: Vec3) -> i32 {
    const R: f32 = 3.0;
    let d = p - eye;
    if d.dot(forward) < -R {
        return 0;
    }
    let past = d.length() - R;
    if R * 0.5 < past { (k / past * R) as i32 } else { (k * 2.0) as i32 }
}

/// Which car draw picks the LOD: the in-game one (0x620AF0, the player and
/// the racers; thresholds 0x7A66E4) or the front end's (0x6162F0, which
/// also draws traffic in game, 0x6317F5; thresholds 0x7A66F4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LodDraw {
    InGame,
    FrontEnd,
}

/// The car draw's LOD (0x620AF0 at 0x620C7B…0x62121F, 0x6162F0 at
/// 0x616307…0x6167CB): none (the car is not drawn) below a size of 4;
/// else from `start` (0x8A1C10: Car Geometry Detail 0x870CF8 Low → 1, High
/// → 0, 0x5D28E0) on while the size is below the threshold (fcomp, `test
/// ah, 5; jnp`: C0 set, so equal stops) — in game
/// 40, 30, 20, 0, the front end's 40, 20, 10, 0 — at most 3 (LOD D). In
/// game a showcase (0x499DB0) takes LOD A. A car with no wheel down whose
/// underside was seen is then kept at C ([`airborne_lod`], the caller's).
/// **Not ported**: split screen (g_ScreenLayout 3: size × 0.7, start 1),
/// the debug overrides 0x802728.
pub fn pick_lod(size: i32, start: u8, draw: LodDraw, showcase: bool) -> Option<u8> {
    if size < 4 {
        return None;
    }
    let table: [f32; 5] = match draw {
        LodDraw::InGame => [40.0, 30.0, 20.0, 0.0, 40.0],
        LodDraw::FrontEnd => [40.0, 20.0, 10.0, 0.0, 18.0],
    };
    let mut lod = start as usize;
    while lod < table.len() && (size as f32) < table[lod] {
        lod += 1;
    }
    if draw == LodDraw::InGame && showcase {
        return Some(0);
    }
    Some(lod.min(3) as u8)
}

/// The airborne rule after [`pick_lod`] (**verified**: 0x620AF0
/// at 0x6211CB…0x6211F8, 0x6162F0 at 0x616776…0x61679E): LOD D becomes C
/// when the car has no wheel on the ground (car +0x424 = 0) and its
/// underside was visible on the car's previous draw (render info +0x2EB8,
/// [`underside_visible`]; 0x60C330 writes it after the pick, 0x621321 /
/// 0x616840, so the pick reads the last draw's). The byte starts at 0
/// (0x638380 zeroes the block).
pub fn airborne_lod(lod: u8, wheels_down: u32, underside_seen: bool) -> u8 {
    if lod > 2 && wheels_down == 0 && underside_seen { 2 } else { lod }
}

/// Record 10 of the cullable car parts, CULLABLE_CAR_PART_UNDERNEATH
/// (table 0x803AE0 + 0x50·10: mode 1, two planes): the normals in car
/// space and their offsets (0x803E0C…0x803E47).
const UNDERSIDE_PLANES: [([f32; 3], f32); 2] = [([-0.07, 0.0, 1.0], 0.05), ([0.07, 0.0, 1.0], 0.05)];

/// 0x60C330 for the underside (record 10, **verified**): `eye` is the
/// camera in car space (0x621240…0x621266: the car matrix inverted by
/// 0x43C7C0, the eye moved by 0x43C9C0), `wheels` the four wheel points
/// of the tuned handling block (0x5B5360(1) +0x10 / +0x40 / +0x70 / +0xA0;
/// 0x638A4C…0x638B8B puts the part at their mean). Outside the game
/// (g_GameState 0x8654A4 ≠ 6) every part counts as visible. In game the
/// mode-1 test: visible only while the eye is below both planes, i.e.
/// n·(eye − point) − d < 0 for each (0x60C5D8…; the normals are used as
/// stored: 0x8A1BD8, which would normalise them, is never written).
pub fn underside_visible(eye: Vec3, wheels: [Vec3; 4], in_game: bool) -> bool {
    if !in_game {
        return true;
    }
    let point = ((wheels[0] + wheels[3]) * 0.5 + (wheels[2] + wheels[1]) * 0.5) * 0.5;
    let d = eye - point;
    UNDERSIDE_PLANES.iter().all(|&(n, offset)| Vec3::from(n).dot(d) - offset < 0.0)
}

/// LOD Fade's hysteresis (clone deviation, `qol::CAR_LOD_HYSTERESIS`):
/// the LOD [`pick_lod`] gives at `size`, unless `current` (the LOD drawn
/// now; None not drawn) is what it gives somewhere between `size × h` and
/// `size / h` — then `current` stays, so a car near a threshold does not
/// switch back and forth.
pub fn pick_lod_kept(size: f32, start: u8, draw: LodDraw, current: Option<u8>, h: f32) -> Option<u8> {
    let pick = |s: f32| pick_lod(s as i32, start, draw, false);
    // Not drawn ranks after LOD D.
    let rank = |l: Option<u8>| l.map_or(4, i32::from);
    if (rank(pick(size * h))..=rank(pick(size / h))).contains(&rank(current)) { current } else { pick(size) }
}
