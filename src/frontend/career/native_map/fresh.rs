//! The original's new profile, written from its rules (R8; docs/original/exe-tables.md,
//! "The fresh profile").
//!
//! SPEED2.EXE builds a new profile at boot (default name "NAME", before any
//! career) from code defaults and the player's game data; the clone used to
//! carry that file. It is now written here, step by step as the original's
//! constructors fill the objects the save writer 0x52A290 then saves:
//! * the garage (0x52A8B0): the stock list (0x516500: every car type with
//!   +0x40 set, +0xDA bit 1 set and +0x844 = 0, as {id
//!   bin_hash("STOCK_<name>"), usable, filter 1, its car type index}; the
//!   other entries free: filter 1, index −1), 20 cleared My Cars records
//!   (filter 2), the five career slots (0x516480: bin_hash
//!   ("CAREER_SLOT_<n>"), usable, filter 4), the six ranked rides
//!   (0x52A800 on the table 0x77D3B0 fills at start: bin_hash of
//!   ONLINE_CAREER_*, a stock car by its car type index, a number kept at
//!   memory +0x434; 0x516B90 makes each a stock record of that car, filter
//!   0x10) and the slot count 5; every car record cleared with its five
//!   dyno settings at 100 (0x516A60);
//! * the career: each stage record's definition number (+0xFD), the
//!   current slot bin_hash("CAREER_0"), the driving time −1, the World
//!   Map legend all shown;
//! * the profile: the EA Trax playlist (index, on), the settings block
//!   (0x5116E0), the Quick Race records (0x515800 / 0x515750; the cars
//!   0x7FAA48 / 0x7FAA4C name by car type index), difficulty 1 and save
//!   prompts on (0x515AA0), the name, 0x850080's two flags, the keyboard's
//!   controller block (GUID_SysKeyboard).
//!
//! The ranked rides' ratings (head +8) are the set-up's area (0x5B4DD0)
//! over the curve 0x5B5AC0 builds, rounded as the game rounds it
//! ([`setup_rating`]).

use crate::formats::hash::bin_hash;
use crate::formats::native_save::*;
use crate::frontend::career::*;

/// One saved car head (memory +4..+0x18): +4 0, +8 id, +0xC rating,
/// +0x10 usable, +0x14 filter.
fn head(p: &mut [u8], at: usize, id: u32, usable: u32, filter: u32) {
    for (k, v) in [0, id, 0, usable, filter].into_iter().enumerate() {
        p[at + 4 * k..at + 4 * k + 4].copy_from_slice(&v.to_le_bytes());
    }
}

/// A cleared car record (0x516A60): five dyno settings of 100.
fn cleared_record(p: &mut [u8], rec: usize) {
    p[rec + REC_TUNING..rec + REC_TUNING + 5 * TUNING_LEN].fill(100);
}

fn put(p: &mut [u8], at: usize, v: u32) {
    p[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

/// The ranked rides (0x77D3B0's table, 12 bytes from 0x8641AC: the name's
/// hash, the stock car's type index, the number 0x52A800 keeps at memory
/// +0x434 — the previous entry's third dword, 0 for the first).
fn ranked_rides() -> Vec<(u32, u32, u32)> {
    let ints = &crate::exe_tables::tables().ranked_rides;
    (0..6).map(|k| (ints[1 + 3 * k], ints[2 + 3 * k], ints[3 * k])).collect()
}

/// The set-up's rating of a stock car type record (0x5B4DD0, kept at the
/// set-up's +8 and returned by 0x5B69A0): from a standstill in steps of
/// 1.3410819 m/s while the speed is within the curve's span, the
/// acceleration curve (0x5764E0) less the drag v·(+0x370)·v / mass, summed
/// × the step while the curve stays ahead (single precision, as the x87
/// runs under Direct3D; the curve rounded as [`setup_curve`] does).
fn setup_rating(record: &[u8]) -> f32 {
    let h = crate::handling::parse_handling(record);
    if h.fixed_class {
        return 0.0;
    }
    let (curve, span) = setup_curve(&h);
    let inv = 63.0 / span;
    let step = crate::handling::PERFORMANCE_STEP;
    let (mut v, mut area) = (0.0f32, 0.0f32);
    while v <= span {
        let a = graph_at(&curve, inv, v);
        let drag = v * h.drag * v / h.mass;
        if a < drag {
            break;
        }
        area = (a - drag) * step + area;
        v += step;
    }
    area
}

/// 0x5764E0 on a graph from x = 0 with `inv` = (n − 1) / span as stored:
/// f = x · inv as an f32, i = trunc(f), the last value from i ≥ n − 1, else
/// (1 − t)·v[i] + t·v[i+1] with t = f − i.
fn graph_at(values: &[f32], inv: f32, x: f32) -> f32 {
    let n = values.len();
    let f = x * inv;
    if !(f >= 0.0) || n < 2 {
        return values.first().copied().unwrap_or(0.0);
    }
    let i = f as usize;
    if i >= n - 1 {
        return values[n - 1];
    }
    let t = f - i as f32;
    (1.0 - t) * values[i] + t * values[i + 1]
}

/// The set-up's 64-point acceleration curve (0x5B5AC0) as the game rounds
/// it: the table from a standstill in 3 mph steps through the gears (as
/// `handling`'s curve), its last entry 0, then sampled at k · span / 63
/// through a graph of the table's n entries with (n − 1) / span stored.
fn setup_curve(h: &crate::handling::Handling) -> (Vec<f32>, f32) {
    let g = &h.gearbox;
    let split = g.rear_split;
    let final_drive = split * g.final_drive_rear + (1.0 - split) * g.final_drive_front;
    let inverse_radius = 1.0 / ((1.0 - split) * h.wheel_radii[0] + split * h.wheel_radii[1]);
    let (up, _) = crate::handling::shift_points(h);
    let mut table = Vec::new();
    let (mut v, mut gear) = (0.0f32, 2usize);
    while table.len() < 0x5C && gear < g.ratios.len() && gear < up.len() {
        let rpm = inverse_radius * v * g.ratios[gear] * final_drive * crate::handling::RPM_PER_RAD_S;
        if up[gear] <= rpm {
            gear += 1;
            if gear >= g.ratios.len() {
                break;
            }
        }
        table.push(h.engine.torque.at(rpm) * g.ratios[gear] * final_drive * inverse_radius / h.mass);
        v += crate::handling::PERFORMANCE_STEP;
    }
    let span = v;
    if let Some(last) = table.last_mut() {
        *last = 0.0;
    }
    let inv = (table.len() as f32 - 1.0) / span;
    let curve = (0..64).map(|k| graph_at(&table, inv, k as f32 * span / 63.0)).collect();
    (curve, span)
}

/// The payload of a new profile named "NAME", from the player's game data
/// (`types`: GLOBALB's car type records in order, `ctx` for the stock
/// parts) and the engine's tables ([`crate::exe_tables`]).
pub fn fresh_payload(types: &[(String, Vec<u8>)], ctx: &NativeContext) -> Vec<u8> {
    let mut p = vec![0u8; PAYLOAD_LEN];
    // The stock list (0x516500).
    let mut stock = Vec::new();
    for (i, (name, r)) in types.iter().enumerate() {
        let flag = |o: usize| r.get(o).copied().unwrap_or(0);
        let kind = u32::from_le_bytes(r[0x844..0x848].try_into().unwrap());
        if flag(0x40) != 0 && flag(0xDA) & 2 != 0 && kind == 0 && stock.len() < LIST_MAX {
            stock.push((i, name.clone()));
        }
    }
    for k in 0..LIST_MAX {
        let at = GARAGE_LIST + k * LIST_LEN;
        match stock.get(k) {
            Some((i, name)) => {
                head(&mut p, at, bin_hash(&format!("STOCK_{name}")), 1, 1);
                put(&mut p, at + 0x14, *i as u32);
            }
            None => {
                head(&mut p, at, 0, 0, 1);
                put(&mut p, at + 0x14, u32::MAX);
            }
        }
    }
    put(&mut p, LIST_COUNT, stock.len() as u32);
    let stock_id = |type_index: u32| {
        stock.iter().find(|(i, _)| *i as u32 == type_index).map_or(0, |(_, n)| bin_hash(&format!("STOCK_{n}")))
    };
    // My Cars (0x52A8B0): filter 2, cleared records.
    for k in 0..GARAGE_CAR_MAX {
        let at = GARAGE_CARS + k * GARAGE_CAR_LEN;
        head(&mut p, at, 0, 0, 2);
        cleared_record(&mut p, at + 0x14);
    }
    // The career slots (0x516480).
    for i in 0..5 {
        let at = SLOTS + i * SLOT_LEN;
        head(&mut p, at, slot_id(i), 1, 4);
        cleared_record(&mut p, at + SLOT_RECORD);
        // Every slot but the first starts locked (0x516480).
        p[at + SLOT_LOCKED] = (i > 0) as u8;
    }
    put(&mut p, SLOT_COUNT, 5);
    // The ranked rides (0x52A800 → 0x516B90).
    for (k, (id, type_index, kept)) in ranked_rides().into_iter().enumerate() {
        let at = RANKED_CARS + k * RANKED_CAR_LEN;
        head(&mut p, at, id, 1, 0x10);
        let rec = at + 0x14;
        put(&mut p, rec, 1);
        let car = types.get(type_index as usize).map_or("", |t| t.0.as_str());
        // The ride's rating (head +8): 0x5B69A0 → the set-up 0x5B4DD0's
        // area between the stock acceleration curve and the drag.
        if let Some((_, record)) = types.get(type_index as usize) {
            put(&mut p, at + 8, setup_rating(record).to_bits());
        }
        put(&mut p, rec + REC_CAR, bin_hash(car));
        for slot in 0..PART_SLOTS {
            put(&mut p, rec + REC_PARTS + 4 * slot, ctx.new_record_part(car, slot));
        }
        cleared_record(&mut p, rec);
        put(&mut p, rec + RECORD_LEN + 4, kept);
    }
    // The ranked rides' count (garage +0x9A80, 0x52A800).
    put(&mut p, GARAGE_TAIL, 6);
    // The career (0x514530): stage definitions, current slot, driving
    // time, the World Map legend.
    for k in 0..STAGE_COUNT {
        let n = ctx.stage_numbers.get(k).copied().unwrap_or(0);
        put(&mut p, STAGES + k * STAGE_LEN + 0xFD, n as i8 as i32 as u32);
    }
    put(&mut p, CURRENT_SLOT, bin_hash("CAREER_0"));
    put(&mut p, DRIVE_TIME, (-1.0f32).to_bits());
    p[LEGEND_SHOWN..LEGEND_SHOWN + 16].fill(1);
    // The profile: the EA Trax playlist, every track on where it plays:
    // 1 the front end ("FE"), 2 in game ("IG").
    for (i, w) in crate::exe_tables::tables().ea_trax_where.iter().enumerate() {
        put(&mut p, TRAX + 8 * i, i as u32);
        put(&mut p, TRAX + 8 * i + 4, if *w == "IG" { 2 } else { 1 });
    }
    // The settings block S (0x5116E0), saved as S+0x24.. at VOLUMES,
    // S+0x14.. at SETTINGS_14, S+0x60.. at PLAYERS, S+4.. at SETTINGS_4.
    let s = |o: usize| match o {
        0x24..0x60 => VOLUMES + o - 0x24,
        0x14..0x24 => SETTINGS_14 + o - 0x14,
        0x60..0xB0 => PLAYERS + o - 0x60,
        _ => SETTINGS_4 + o - 4,
    };
    put(&mut p, s(4), 0.5f32.to_bits());
    p[s(0x14)] = 1;
    p[s(0x17)] = 2;
    p[s(0x18)] = 2;
    put(&mut p, s(0x1C), 127.5f32.to_bits());
    put(&mut p, s(0x20), 127.5f32.to_bits());
    for (k, v) in [1.0f32, 1.0, 0.7, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0].into_iter().enumerate() {
        put(&mut p, s(0x24 + 4 * k), v.to_bits());
    }
    put(&mut p, s(0x4C), 1);
    put(&mut p, s(0x54), 1);
    for pl in 0..2 {
        let b = 0x60 + 0x28 * pl;
        for o in [0, 1, 2, 3, 4, 5, 6, 0x16, 0x18] {
            p[s(b + o)] = 1;
        }
        put(&mut p, s(b + 0x10), 3);
        put(&mut p, s(b + 0x1C), 1.0f32.to_bits());
        put(&mut p, s(b + 0x20), 12.4f32.to_bits());
        put(&mut p, s(b + 0x24), 1.0f32.to_bits());
    }
    // The Quick Race records (0x515800, then 0x515750 on its 13th entry).
    // 13 entries of 0x14 from +0x10: bytes 1, 3, 1, 1, 1, 1, 0; the u16 at
    // +0xA is 3 in the first twelve before 0x515800's own values
    // (**inferred** from the saved file: the record's constructor is not
    // traced).
    let r = RACE_RECORD;
    for k in 0..13 {
        let e = r + 0x10 + 0x14 * k;
        p[e..e + 7].copy_from_slice(&[1, 3, 1, 1, 1, 1, 0]);
        if k < 12 {
            p[e + 0xA..e + 0xC].copy_from_slice(&3u16.to_le_bytes());
        }
    }
    let w16 = |p: &mut Vec<u8>, o: usize, v: u16| p[r + o..r + o + 2].copy_from_slice(&v.to_le_bytes());
    w16(&mut p, 0x2E, 1);
    w16(&mut p, 0x42, 1);
    p[r + 0x75] = 1;
    w16(&mut p, 0x18, 0xFB5);
    w16(&mut p, 0x56, 3);
    p[r + 0x50] = 2;
    w16(&mut p, 0xCE, 0);
    p[r + 0xC5] = 0;
    p[r + 0x76] = 0;
    p[r + 0xDA] = 0;
    w16(&mut p, 0xE0, 0x11F9);
    p[r + 0xDC] = 0;
    w16(&mut p, 0xE2, 10);
    p[r + 0x61] = 5;
    put(&mut p, r + 0x108, 2);
    put(&mut p, r + 0x10C, 0);
    // 0x515750 on the entry at +0x100.
    let b = r + 0x100;
    p[b..b + 6].copy_from_slice(&[1, 3, 1, 1, 1, 3]);
    p[b + 0x19] = 1;
    p[b + 0x1A] = 0;
    put(&mut p, b + 0x14, 0);
    for k in 0..8 {
        let o = b + 0x1C + 0xC * k;
        p[o..o + 2].copy_from_slice(&0u16.to_le_bytes());
        p[o + 2..o + 4].copy_from_slice(&3u16.to_le_bytes());
        put(&mut p, o + 4, 0);
        p[o + 8] = 0;
        p[o + 9] = 0;
    }
    let cars = crate::exe_tables::tables().qr_default_cars;
    put(&mut p, r + 4, stock_id(cars[0]));
    put(&mut p, r + 8, stock_id(cars[1]));
    // Difficulty 1, save prompts on (0x515AA0).
    p[DIFFICULTY] = 1;
    p[SAVE_PROMPTS] = 1;
    // 0x850080 (0x515BF0): +0x49 and +0x4A set.
    p[OBJ_850080 + 8] = 1;
    p[OBJ_850080 + 9] = 1;
    // The keyboard's controller block.
    p[CONTROLS..CONTROLS + 16].copy_from_slice(&GUID_SYS_KEYBOARD);
    let mut save = NativeSave { payload: p };
    save.set_name("NAME");
    save.payload
}

/// A new profile named `name`: [`fresh_profile`] renamed (what a profile
/// the clone creates starts from).
pub fn fresh_save(name: &str) -> NativeSave {
    NativeSave::fresh(fresh_profile(), name)
}

/// The new profile (the payload of a profile named "NAME"), written once
/// per process from the player's game folder ([`crate::game_root`]) the
/// first time a profile is created.
pub fn fresh_profile() -> &'static NativeSave {
    static FRESH: std::sync::OnceLock<NativeSave> = std::sync::OnceLock::new();
    FRESH.get_or_init(|| {
        let root = crate::game_root();
        let globalb = crate::game_fs::read(&root, "GLOBAL/GLOBALB.BUN").unwrap_or_default();
        let types = crate::car::car_type_records(&globalb).unwrap_or_default();
        let built;
        let ctx = match NativeContext::get() {
            Some(c) => c,
            None => {
                built = NativeContext::new(
                    &root,
                    &CareerData::load(&root).unwrap_or_else(|e| panic!("the career data: {e:#}")),
                );
                &built
            }
        };
        NativeSave { payload: fresh_payload(&types, ctx) }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The written profile is the one the original writes, byte for byte:
    /// the payload of the file the repository carried
    /// (`native_save_fresh.bin`, 54,966 bytes, SHA-256
    /// e623c3acb3523f2cae3db41fd43a0ad983af16c2e471454d6dd513997ad4c1e3),
    /// pinned by its SHA-256.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn the_written_profile_is_the_originals() {
        const PAYLOAD: &str = "20159de64ed31712821b9ccd8a769e48a4bc39db7f25556482db2595c9ab5216";
        const FILE: &str = "e623c3acb3523f2cae3db41fd43a0ad983af16c2e471454d6dd513997ad4c1e3";
        let root = crate::testkit::game_root();
        let data = CareerData::load(&root).unwrap();
        let ctx = NativeContext::new(&root, &data);
        let g = crate::game_fs::read(&root, "GLOBAL/GLOBALB.BUN").unwrap();
        let types = crate::car::car_type_records(&g).unwrap();
        let ours = fresh_payload(&types, &ctx);
        assert_eq!(crate::fingerprint::sha256_hex(&ours), PAYLOAD);
        let file = fresh_profile().to_bytes();
        assert_eq!(file.len(), FILE_LEN);
        assert_eq!(crate::fingerprint::sha256_hex(&file), FILE);
        assert_eq!(NativeSave::parse(&file).unwrap().name(), "NAME");
    }
}
