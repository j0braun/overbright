//! Pure part rules over the part database: slot visibility, carbon, decals, animation channels.

use super::*;

/// The parts the performance parts put on a ride (0x6399E0, run by
/// 0x639CF0 whenever a ride's parts or performance change; ride +0x2F0 the
/// performance block, its fitted flags at +0x50 + id): slot 0x0D (ENGINE)
/// takes the level-3 part when the engine, ECU and turbo packages of level
/// 3 (categories 3, 7, 4; 0x599640 — a package's parts all fitted, or no
/// such package) are all on, else level 2 for the level-2 ones, else level
/// 0; slot 0x1B (the brakes, front and rear models) the level 1 part with
/// performance part 0x28 (the level-1 brakes' third), else 2 with 0x29,
/// else 3 with 0x2B, else level 0 (0x599620 each). 0x61BC90 finds the
/// car's first part of the slot at that level; with none the slot keeps
/// its part.
pub fn perf_visual_parts(
    installed: &std::collections::BTreeMap<u8, u32>,
    db: &PartDb,
    car: &str,
    packages: &[crate::performance::Package],
    fitted: &dyn Fn(u8) -> bool,
) -> std::collections::BTreeMap<u8, u32> {
    let mut out = installed.clone();
    // No performance data (a car loaded without it): nothing to apply.
    if packages.is_empty() {
        return out;
    }
    let package_on = |category: u8, level: u8| {
        packages
            .iter()
            .find(|p| p.category == category && p.level == level)
            .is_none_or(|p| p.parts.iter().all(|x| fitted(x.id)))
    };
    let all = |level: u8| [3, 7, 4].iter().all(|&c| package_on(c, level));
    let engine = if all(3) {
        3
    } else if all(2) {
        2
    } else {
        0
    };
    let brakes = if fitted(0x28) {
        1
    } else if fitted(0x29) {
        2
    } else if fitted(0x2B) {
        3
    } else {
        0
    };
    for (slot, level) in [(0x0Dusize, engine), (0x1B, brakes)] {
        let mut after = None;
        while let Some(p) = db.find_after(car, slot, after) {
            after = Some(p);
            if p.level == level {
                out.insert(slot as u8, p.hash);
                break;
            }
        }
    }
    out
}

/// The parts 0x61BCE0 (run by 0x6399E0 after it sets slot 0x0D / 0x1B,
/// so on every 0x639CF0) derives from others, as `(slot, part hash)`; a slot whose lookup finds
/// nothing is `0` (the ride's slot is then empty). `part(slot)` is the
/// ride's part hash in a slot (the stock one where nothing else is fitted).
/// * Slot 6 (WIDE_BODY) with no part or a level-0 part: slots 0x11–0x16
///   (the doors, door panels and door sills) take the car's first part of
///   the slot (0x61B990 with hash 0, any level).
/// * Slot 6 of level ≥ 1, `n` its kind (part byte +5 & 0x1F): slots
///   0x3B–0x3E take `<CAR>_WIDE<n>_DECAL_{LEFT,RIGHT}_{DOOR,QUARTER}_RECT_MEDIUM`
///   (0x61BF6B formats "%s_WIDE%d_", then 0x43DB80 appends) and slots
///   0x11–0x16 `<CAR>_KITW0<n>_DOOR_` + LEFT / RIGHT / PANEL_LEFT /
///   PANEL_RIGHT / SILL_LEFT / SILL_RIGHT (0x61C02B, "%s_KITW0%d_DOOR_").
/// * Slot 9 (the hood) holding a part: slot 0x19 (HOOD_UNDER) takes the
///   part its HOODUNDER attribute names, or the first part without one.
/// **Verified** by code (0x61BCE0) and against the original's save after a
/// purchase (a community save's LANCEREVO8, wide body kind 4: slots 0x16, 0x19 and
/// 0x3B–0x3E rewritten). The slot flags 0x61BCE0 also sets (ride +0x838…)
/// are the draw's (`PART_TYPE_NAMES`).
pub fn ride_slot_parts(db: &PartDb, car: &str, part: &dyn Fn(usize) -> Option<u32>) -> Vec<(u8, u32)> {
    let find = |slot: usize, hash: u32| db.find(car, slot, (hash != 0).then_some(hash), None).map_or(0, |p| p.hash);
    let lookup = |slot: usize| part(slot).and_then(|h| db.find(car, slot, Some(h), None));
    let mut out = Vec::new();
    match lookup(6).filter(|p| p.level != 0) {
        None => out.extend((0x11..=0x16).map(|slot| (slot as u8, find(slot, 0)))),
        Some(wide) => {
            let decal = bin_hash(&format!("{car}_WIDE{}_", wide.kind));
            for (slot, name) in [
                (0x3B, "DECAL_LEFT_DOOR_RECT_MEDIUM"),
                (0x3C, "DECAL_RIGHT_DOOR_RECT_MEDIUM"),
                (0x3D, "DECAL_LEFT_QUARTER_RECT_MEDIUM"),
                (0x3E, "DECAL_RIGHT_QUARTER_RECT_MEDIUM"),
            ] {
                out.push((slot as u8, find(slot, bin_hash_from(decal, name))));
            }
            let door = bin_hash(&format!("{car}_KITW0{}_DOOR_", wide.kind));
            for (slot, name) in [
                (0x11, "LEFT"),
                (0x12, "RIGHT"),
                (0x13, "PANEL_LEFT"),
                (0x14, "PANEL_RIGHT"),
                (0x15, "SILL_LEFT"),
                (0x16, "SILL_RIGHT"),
            ] {
                out.push((slot as u8, find(slot, bin_hash_from(door, name))));
            }
        }
    }
    if let Some(hood) = lookup(9) {
        out.push((0x19, find(0x19, db.attribute(hood, bin_hash("HOODUNDER")).unwrap_or(0))));
    }
    out
}

/// A ride's fitted parts (absent = stock) with 0x61BCE0's derived slots
/// ([`ride_slot_parts`]) put in, as every ride the game builds by
/// 0x631D30 holds them (the career ride from a record, 0x503880, even
/// when the record's own derived slots are stale); a lookup that finds
/// nothing keeps the slot's part, stock parts stay implicit.
pub fn with_derived_slots(
    installed: &std::collections::BTreeMap<u8, u32>,
    db: &PartDb,
    car: &str,
) -> std::collections::BTreeMap<u8, u32> {
    let mut parts = installed.clone();
    let stock = |slot: usize| db.stock(car, slot).map(|p| p.hash);
    for (slot, h) in ride_slot_parts(db, car, &|slot| installed.get(&(slot as u8)).copied().or_else(|| stock(slot))) {
        if h != 0 {
            parts.insert(slot, h);
        }
    }
    parts.retain(|&slot, h| Some(*h) != stock(slot as usize));
    parts
}

/// The visual rating (0x5234D0) of `car` fitted with `ride` (slot → part
/// hash, absent = the stock part, the paint absent = the car type's stock
/// paint `stock_paint`, car type +0x84C) as 0x528420 rates the career car:
/// the ride built from the record (0x503880, with the derived slots), then
/// every slot's part points ([`PartDb::visual_rating`]).
pub fn ride_visual_rating(
    db: &PartDb,
    costs: &crate::formats::parts::CostTable,
    car: &str,
    stock_paint: u32,
    ride: &std::collections::BTreeMap<u8, u32>,
) -> f32 {
    let ride = with_derived_slots(ride, db, car);
    let fitted = |slot: usize| match ride.get(&(slot as u8)) {
        Some(&h) => db.find(car, slot, Some(h), None).copied(),
        None if slot == 0x3F => db.find(car, slot, Some(stock_paint), None).copied(),
        None => db.stock(car, slot).copied(),
    };
    db.visual_rating(costs, stock_paint, &fitted)
}

/// The part 0x5474B0 fits in a trunk audio component slot (0x23–0x2E)
/// when a trunk layout (slot 0x22) is fitted: 0x53AEB0's, the slot's
// prov: inferred car.trunk_component
/// first part of level 0 that 0x52A020 passes (**inferred**: every
/// level-0 part passes), or none.
pub fn trunk_component(db: &PartDb, car: &str, slot: usize) -> Option<u32> {
    let mut after = None;
    while let Some(p) = db.find_after(car, slot, after) {
        after = Some(p);
        if p.level == 0 {
            return Some(p.hash);
        }
    }
    None
}

/// The carbon-fibre part slots: KIT_CARBON (0x5D, ride +0x704) and
/// TRUNK_CARBON (0x60, +0x710); a part above level 0 ("CARBON FIBRE",
/// level 3) turns them on.
pub const KIT_CARBON: usize = 0x5D;
pub const TRUNK_CARBON: usize = 0x60;
/// `CARBONFIBRE` (0x7A8528): the texture and the hood attribute key.
pub const CARBONFIBRE: u32 = 0x721A_FF7C;
/// `CARBONFIBER` (0x7A8D2C): the light material the car's render info
/// holds at +0x388 (0x638DA9…0x638DBF, through 0x43DB50 / 0x48F6A0) for
/// the carbon parts (docs/car-materials.md).
pub const CARBONFIBER_MATERIAL: u32 = 0x721A_FDDC;

/// 0x634B0B…0x634E69: the table a slot's model takes. `above` is "the
/// slot holds a part above level 0" (part byte +5 >> 5); `carbon_hood`
/// the hood part's `CARBONFIBRE` attribute being non-zero (0x634C95).
pub fn carbon_for(slot: usize, above: &dyn Fn(usize) -> bool, carbon_hood: bool) -> Carbon {
    let kit = above(KIT_CARBON);
    let full = |on: bool| if on { Carbon::Full } else { Carbon::None };
    match slot {
        // Front and rear bumper (+0x3B0, +0x3B4).
        1 | 2 => full(kit),
        // The wide body, with the kit on (+0x3C8).
        6 => full(above(6) && kit),
        // The hood (+0x3BC).
        9 => full(carbon_hood),
        // The trunk (+0x3C4): its own carbon, else the kit's partial table.
        0xA if above(TRUNK_CARBON) => Carbon::Full,
        // Doors 0x11 / 0x12 on a wide body (+0x3C8).
        0x11 | 0x12 if above(6) => full(kit),
        // Body, skirt, and the doors without a wide body (+0x3B8).
        5 | 0xA | 0xB | 0x11 | 0x12 => {
            if kit {
                Carbon::Partial
            } else {
                Carbon::None
            }
        }
        _ => Carbon::None,
    }
}

/// `DUMMY_DECAL1`…`8`: the keys of each decal block (0x61FF50).
pub fn dummy_decal(j: usize) -> u32 {
    bin_hash(&format!("DUMMY_DECAL{}", j + 1))
}

/// `DEFAULTALPHA` (GLOBALB, 32² DXT3): an empty decal entry's texture.
pub const DEFAULT_ALPHA: &str = "DEFAULTALPHA";

/// 0x61FF50: decal location slot `slot`'s eight textures. The block is
/// slot − 0x34, less 4 for the wide slots 0x3B–0x3E (0x634AE1), so the
/// wide doors and quarters share the plain ones' decals. Every entry
/// starts as `DEFAULTALPHA`; then for placement slots 0x34–0x3A (0x34
/// only with a hood part, ride +0x5B4) whose part has a SIZE attribute
/// and a SHAPE, entry j takes decal slot 0x61 + 8·block + j's part's NAME
/// attribute (0x0019CBC0, a hash) continued with `_SQUARE`, `_RECT` or
/// `_WIDE` by the SHAPE (no suffix for another shape).
/// `attr(slot, key)` is the fitted part's attribute.
pub fn decal_block(slot: usize, attr: &dyn Fn(usize, u32) -> Option<Option<u32>>, has_hood: bool) -> [u32; 8] {
    let empty = bin_hash(DEFAULT_ALPHA);
    let mut out = [empty; 8];
    let block = slot - 0x34 - if slot >= 0x3B { 4 } else { 0 };
    let placement = 0x34 + block;
    if placement == 0x34 && !has_hood {
        return out;
    }
    let Some(Some(_)) = attr(placement, bin_hash("SIZE")) else { return out };
    let Some(Some(shape)) = attr(placement, bin_hash("SHAPE")) else { return out };
    let suffix = if shape == bin_hash("SQUARE") {
        "_SQUARE"
    } else if shape == bin_hash("RECT") {
        "_RECT"
    } else if shape == bin_hash("WIDE") {
        "_WIDE"
    } else {
        ""
    };
    for (j, o) in out.iter_mut().enumerate() {
        // A fitted decal without a NAME hashes on from 0 (0x6203E9).
        if let Some(name) = attr(0x61 + 8 * block + j, bin_hash("NAME")) {
            *o = bin_hash_from(name.unwrap_or(0), suffix);
        }
    }
    out
}

/// Which model of a slot a part animation channel moves (GLOBALB chunk
/// 0x34608, loaded at 0x636C69 into 0x8A1CDC: a signed byte per slot·2 +
/// group, −1 none), as the generic part draw reads it (0x62345B):
/// HOOD (9), slot 0x19 and DECAL_HOOD (0x34) the hood; TRUNK (0xA),
/// slot 0x1A, SPOILER's (0xC) group 0 and BRAKELIGHT's (0xF) group 1 the
/// trunk; the door slots 0x11 / 0x12, 0x13 / 0x14 and the door decals
/// 0x37 / 0x38, 0x3B / 0x3C the left / right doors. The draw poses the
/// model while the channel is not shut (0x6234E2) as
/// T(a − p) · A · [R⁻¹] · T(p − r) in car space (0x6235A1…0x6243E5): A the
/// animation's turn, R the rest matrix (r its translation), a the
/// animation's translation (r when it has none, 0x434190), p the solid's
/// +0x70; R⁻¹ for the hood and the doors' SCISSOR / WIDE animations only
/// (0x623A7A, 0x434550). The spoiler has its own draw (0x625AAB), so its
/// entry is unused; that draw puts its group 0 on the trunk's posed matrix
/// instead (0x6243F4 keeps it), and the mirrors on the doors'. See
/// [`animated_channel`] for slots 0x36 and 0xE.
pub const ANIMATED: u32 = 0x0003_4608;

/// 0x62345B…0x62355F: the channel the generic part draw poses slot
/// `slot`'s group `group` model by, from its [`ANIMATED`] entry `entry`,
/// and the group it evaluates the pose and rest matrix for (0x434190 /
/// 0x434460's fourth argument). Slot 0x36 (DECAL_REAR_WINDOW) takes the
/// trunk on a car type 0x60C900 holds for (the hatchbacks and SUVs: 0–3,
/// 6–11, 13, 15–20, 22, 24, 29), and slot 0xE (HEADLIGHT) the hood on car
/// type 8 (HUMMER), as its group 1 when the hood is VERTSPLIT (render info
/// +0x2D48).
pub fn animated_channel(slot: usize, group: usize, entry: i8, type_index: usize, vertsplit: bool) -> (Option<u8>, u8) {
    let hatch = matches!(type_index, 0..=3 | 6..=11 | 13 | 15..=20 | 22 | 24 | 29);
    match slot {
        0x36 if hatch => (Some(1), group as u8),
        0x0E if type_index == 8 => (Some(0), if vertsplit { 1 } else { group as u8 }),
        _ => ((entry >= 0).then_some(entry as u8), group as u8),
    }
}

/// Which slots the generic part draw shows (ride +0x838, set up by
/// 0x61BCE0, tested at 0x62319F): all, less each name a fitted part's
/// attributes 0xE80A3B62 give (not type 0x45); then with a wide body (slot
/// 6, level above 0) the front and rear bumpers, the body, the skirt, slots
/// 0x17 / 0x18 and the decal slots 0x37–0x3A are hidden and slot 6 and
/// 0x3B–0x3E shown, without one the other way round. `level(slot)` is the
/// fitted part's level, `hides(slot)` its 0xE80A3B62 values.
pub fn shown_slots(level: &dyn Fn(usize) -> Option<u8>, hides: &dyn Fn(usize) -> Vec<u32>) -> [bool; 0xAA] {
    let mut shown = [true; 0xAA];
    for slot in 0..0xAA {
        for h in hides(slot) {
            if let Some(i) = PART_TYPE_NAMES.iter().position(|n| bin_hash(n) == h).filter(|&i| i != 0x45) {
                shown[i] = false;
            }
        }
    }
    let wide = level(6).is_some_and(|l| l > 0);
    for s in [1, 2, 5, 0xB, 0x17, 0x18, 0x37, 0x38, 0x39, 0x3A] {
        shown[s] = !wide;
    }
    for s in [6, 0x3B, 0x3C, 0x3D, 0x3E] {
        shown[s] = wide;
    }
    shown
}
