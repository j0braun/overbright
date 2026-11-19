//! Car-type, preset and camera-view records parsed from GLOBALB.

use super::*;

/// GLOBALB 0x00034600: one 0x890-byte record per car model.
pub(super) const CAR_TYPES: u32 = 0x0003_4600;
pub(super) const CAR_TYPE_SIZE: usize = 0x890;
/// GLOBALB 0x00135200: one 0xA0-byte light material.
pub(super) const LIGHT_MATERIAL: u32 = 0x0013_5200;

#[derive(Clone, Copy, Debug)]
pub struct Wheel {
    /// Wheel centre in car space (x forward, y left, z up).
    pub position: Vec3,
    pub radius: f32,
    /// +0x14: the tyre's width, which an aftermarket rim is drawn at.
    pub width: f32,
    /// +0x1C: the y of the wheel's outer face (0x61FE20 → render info
    /// +0x400, negated), where the wheel draw (0x6162F0) puts it.
    pub face: f32,
    /// +0x20: the face's y with a wide body fitted (0x61FE20 → render
    /// info +0x404, negated; the in-game wheel draw 0x620AF0 reads it in
    /// place of +0x1C when the WIDE_BODY part, slot 6, is above level 0).
    pub face_wide: f32,
}

/// One camera position from the car's own table (record +0x720).
#[derive(Clone, Copy, Debug)]
pub struct CameraView {
    /// 0 far chase, 1 near chase, 2 bumper, 3 driver, 4 hood, 5 high chase.
    pub kind: u16,
    /// Downward tilt as a binary angle (65536 = a full turn).
    pub tilt: u16,
    /// Eye in car space (x forward, y left, z up).
    pub eye: Vec3,
}

#[derive(Clone, Debug)]
pub struct CarType {
    pub name: String,
    /// Record +0xC0: the manufacturer ("FORD").
    pub manufacturer: String,
    /// Front left, front right, rear right, rear left (the table's order).
    pub wheels: [Wheel; 4],
    pub handling: Handling,
    /// The record's handling before any race tune or set-up.
    pub stock: Handling,
    /// The camera table the game reads through 0x5ECCB0 / 0x441140.
    pub cameras: Vec<CameraView>,
    /// Record +0x84C: the hash of the car's stock paint (a part in slot
    /// 0x3F; 0x637050).
    pub stock_paint: u32,
    /// Record +0xDC: the rim size the car takes; rims (part type 0x1C)
    /// fit when their attribute 0xCE7D8DB5 matches (0x539B80).
    pub rim_size: u8,
    /// Record +0xDD: the smallest rim (attribute 0xEB0101E2) the AI rim
    /// draw (0x632DF0) takes for it.
    pub rim_min: u8,
    /// Record +0xDE: the largest rim (attribute 0xEB0101E2) the unique-part
    /// list takes for it (0x533D30).
    pub rim_max: u8,
    /// Record +0x88A: an SUV (the career opens its SUV events, event
    /// +0x0D = 1, only to these; 0x527D40).
    pub suv: bool,
    /// A stock car the car lots can offer (0x516500 makes a `STOCK_<name>`
    /// record for it): its geometry path (+0x40) is set, bit 1 of +0xDA
    /// is set, and +0x844 is 0 (traffic cars, 2, only behind the unset
    /// flag 0x838470).
    pub stock_car: bool,
    /// Record +0x844 = 2: a traffic car (TAXI, BUS, PICKUP …; the type
    /// the stock-car test above excludes).
    pub traffic: bool,
    /// Record +0x854 + zone: how many traffic cars may take this model in
    /// a traffic zone (the index at zone +0x3C, 0x5D9C00 type 0x10); 0 for
    /// AMBULANCE, BUS and FIRETRUCK (0x412BE0, 0x412950).
    pub traffic_caps: [i8; 5],
    /// Record +0x88C ≠ 0: the car gets a `DUMMY_SKIN<n>` composite slot
    /// (0x61C290 sets ride +0x584 only then; 1 for the 31 racer types, 0
    /// for the 15 traffic types). Without it the skin compositor 0x61D7A0
    /// returns at once and the remap (0x631E30) keeps the shipped
    /// `<CAR>_SKIN1` of CARS/TEXTURES.BIN.
    pub composited_skin: bool,
    /// The turbo's rounded upgrade level, 0x5995F0(4) on the car's ride
    /// +0x2F0 (the upgrade state's float for category 4, + 0.5, truncated;
    /// `performance::state_levels`): the shift and rev flame pops
    /// (0x5EBE10, 0x5F5F5F) fire only from level 2. 0 for a stock record.
    pub turbo_level: u8,
}

/// Record +0x720: two counts, then six 16-byte entries {i16 kind, u16 tilt,
/// f32 x, f32 z, f32 y}. A second set at +0x790 is used only in split
/// screen (0x8707DC == 3).
pub(super) fn parse_camera_views(r: &[u8]) -> Vec<CameraView> {
    let count = (read_u32(r, 0x720) as usize).min(6);
    (0..count)
        .map(|i| {
            let e = 0x730 + i * 0x10;
            CameraView {
                kind: read_u16(r, e),
                tilt: read_u16(r, e + 2),
                eye: Vec3::new(read_f32(r, e + 4), read_f32(r, e + 0xC), read_f32(r, e + 8)),
            }
        })
        .collect()
}

/// Parse every car type in GLOBALB, as the physics sees them with no
/// performance parts fitted (the setup 0x5AC500 still runs: the turbo
/// curve joins the torque and the inertia follows the mass).
pub fn parse_car_types(globalb: &[u8]) -> Result<Vec<CarType>> {
    let chunk = bundle::chunks(globalb, 0).find(|c| c.id == CAR_TYPES).context("no car type table")?;
    let body = chunk.payload();
    Ok(body
        .chunks_exact(CAR_TYPE_SIZE)
        .enumerate()
        .map(|(i, r)| {
            let mut r = r.to_vec();
            crate::performance::upgrade(&mut r, &[], &|_| false);
            car_type(&r, i)
        })
        .collect())
}

/// The car-type records as stored (0x890 bytes, no upgrades), with their
/// names: the shops upgrade copies of them to rate the car.
pub fn car_type_records(globalb: &[u8]) -> Result<Vec<(String, Vec<u8>)>> {
    let chunk = bundle::chunks(globalb, 0).find(|c| c.id == CAR_TYPES).context("no car type table")?;
    Ok(chunk.payload().chunks_exact(CAR_TYPE_SIZE).map(|r| (read_cstr(r, 0, 0x20), r.to_vec())).collect())
}

/// One car type with performance parts fitted (0x5AC500): `fitted` says
/// which part ids (0–0x43) the car has.
pub fn parse_car_type_upgraded(
    globalb: &[u8],
    name: &str,
    packages: &[crate::performance::Package],
    fitted: &dyn Fn(u8) -> bool,
) -> Result<CarType> {
    let chunk = bundle::chunks(globalb, 0).find(|c| c.id == CAR_TYPES).context("no car type table")?;
    let (index, r) = chunk
        .payload()
        .chunks_exact(CAR_TYPE_SIZE)
        .enumerate()
        .find(|(_, r)| read_cstr(r, 0, 0x20) == name)
        .with_context(|| format!("no car type {name}"))?;
    let mut r = r.to_vec();
    crate::performance::upgrade(&mut r, packages, fitted);
    let mut c = car_type(&r, index);
    // The race tunes blend by the upgrade state (0x5A7540).
    let state = crate::handling::upgrade_state(packages, fitted);
    c.stock.upgrade_state = state;
    c.handling.upgrade_state = state;
    Ok(c)
}

/// The dyno's tuning to apply after the upgrades (0x5B4DD0 → 0x5B4790):
/// the 0x34A1A table and the car's slider settings.
pub type Tune<'a> = Option<(&'a [crate::tuning::Unlock], &'a crate::tuning::Settings)>;

/// [`parse_car_type_upgraded`], then tuned (`tuning::apply`).
pub fn parse_car_type_tuned(
    globalb: &[u8],
    name: &str,
    packages: &[crate::performance::Package],
    fitted: &dyn Fn(u8) -> bool,
    tune: Tune,
) -> Result<CarType> {
    let chunk = bundle::chunks(globalb, 0).find(|c| c.id == CAR_TYPES).context("no car type table")?;
    let (index, stock) = chunk
        .payload()
        .chunks_exact(CAR_TYPE_SIZE)
        .enumerate()
        .find(|(_, r)| read_cstr(r, 0, 0x20) == name)
        .with_context(|| format!("no car type {name}"))?;
    let mut r = stock.to_vec();
    crate::performance::upgrade(&mut r, packages, fitted);
    let tuned = tune.map(|(_, settings)| settings.0);
    if let Some((table, settings)) = tune {
        crate::tuning::apply(&mut r, stock, table, fitted, settings);
    }
    // Check aid (not native): NFSU2_DUMP_TUNED_RECORDS=DIR writes the
    // physics record as built (parts and tuning applied), for the tuning
    // sweep against the original's in-memory copy (the maintainer's tool `recdump.py`).
    if let Some(dir) = std::env::var_os("NFSU2_DUMP_TUNED_RECORDS") {
        let dir = std::path::Path::new(&dir);
        std::fs::write(dir.join(format!("{name}{}.bin", if tuned.is_some() { "-tuned" } else { "" })), &r).ok();
        if let Some(sliders) = tuned {
            std::fs::write(dir.join(format!("{name}-tuned.sliders")), sliders).ok();
        }
    }
    let mut c = car_type(&r, index);
    c.turbo_level = crate::performance::state_levels(packages, fitted)[4];
    // The race tunes blend by the upgrade state (0x5A7540).
    let state = crate::handling::upgrade_state(packages, fitted);
    c.stock.upgrade_state = state;
    c.handling.upgrade_state = state;
    Ok(c)
}

/// The car type record with the packages fitted (and the dyno tuning),
/// as [`parse_car_type_tuned`] reads it (parity checks against the
/// original's car +0xC copy).
pub fn tuned_record(
    globalb: &[u8],
    name: &str,
    packages: &[crate::performance::Package],
    fitted: &dyn Fn(u8) -> bool,
) -> Result<Vec<u8>> {
    let chunk = bundle::chunks(globalb, 0).find(|c| c.id == CAR_TYPES).context("no car type table")?;
    let stock = chunk
        .payload()
        .chunks_exact(CAR_TYPE_SIZE)
        .find(|r| read_cstr(r, 0, 0x20) == name)
        .with_context(|| format!("no car type {name}"))?;
    let mut r = stock.to_vec();
    crate::performance::upgrade(&mut r, packages, fitted);
    Ok(r)
}

pub(super) fn car_type(r: &[u8], index: usize) -> CarType {
    {
        let mut c = CarType {
            name: read_cstr(r, 0, 0x20),
            manufacturer: read_cstr(r, 0xC0, 0x20),
            wheels: std::array::from_fn(|i| {
                let w = 0x120 + i * 0x30;
                Wheel {
                    position: Vec3::from(read_vec3(r, w)),
                    radius: read_f32(r, w + 0x10),
                    width: read_f32(r, w + 0x14),
                    face: read_f32(r, w + 0x1C),
                    face_wide: read_f32(r, w + 0x20),
                }
            }),
            handling: {
                let mut h = parse_handling(r);
                crate::handling::setup_performance(&mut h, crate::race::RaceMode::FreeRoam);
                h
            },
            stock: parse_handling(r),
            cameras: parse_camera_views(r),
            stock_paint: read_u32(r, 0x84C),
            rim_size: r[0xDC],
            rim_min: r[0xDD],
            rim_max: r[0xDE],
            suv: r[0x88A] != 0,
            stock_car: r[0x40] != 0 && (r[0xDA] >> 1) & 1 != 0 && read_u32(r, 0x844) == 0,
            traffic: read_u32(r, 0x844) == 2,
            traffic_caps: std::array::from_fn(|k| r[0x854 + k] as i8),
            composited_skin: read_u32(r, 0x88C) != 0,
            turbo_level: 0,
        };
        c.handling.type_index = index;
        c.stock.type_index = index;
        c
    }
}

/// A preset ride (GLOBALB chunk 0x30220, 0x338-byte records the game
/// keeps as a list at 0x8A31E4): a named car with its parts, as 0x637280
/// puts one on a racer.
#[derive(Clone, Debug)]
pub struct Preset {
    /// +0x28: its name ("DDAY_PLAYER_CAR"), looked up by `fe_hash`
    /// (0x61C470).
    pub name: String,
    /// +0x08: the car type's name ("350Z").
    pub car: String,
    /// +0x48: the performance level fitted (0x5A64B0: every category's
    /// packages of levels 1 up to it), or −1 for the per-part flags.
    pub perf_level: i32,
    /// +0x4C: a part hash per slot (0xAA): 0 none, 1 the stock part.
    pub parts: Vec<u32>,
    /// +0x2F4: per performance part id (0x44), −1 (0xFF) unset, else
    /// fitted when non-zero.
    pub perf_parts: Vec<u8>,
}

impl Preset {
    /// The parts it fits over the stock ones, by slot: 0x637280 skips
    /// slot 0x19 and slots 0x61–0x68, a value of 1 keeps the stock part
    /// and 0 empties the slot (ride +0x590 + 4·slot = 0; kept here as hash
    /// 0). Slot 8 always takes the car's first part of any level
    /// (0x637351: 0x61B990(car, 8, 0, 0, −1)); the clone keeps its stock
    // prov: inferred car.installed
    /// part, the first of level 0 (**inferred** to be the same part).
    pub fn installed(&self) -> std::collections::BTreeMap<u8, u32> {
        self.parts
            .iter()
            .enumerate()
            .filter(|&(slot, &h)| h != 1 && slot != 8 && slot != 0x19 && !(0x61..=0x68).contains(&slot))
            .map(|(slot, &h)| (slot as u8, h))
            .collect()
    }

    /// The performance part ids it fits: with a level, every package of
    /// that level or lower (0x5A64B0 over categories 0–9 and 0x500100);
    /// otherwise the flagged ids.
    pub fn perf(&self, packages: &[crate::performance::Package]) -> std::collections::BTreeSet<u8> {
        if self.perf_level >= 0 {
            // 0x5A64B0(level, −1): level by level from 1, categories 0–9
            // (0x500100), each package's parts through Perf_FitPart
            // (0x5A6330(part, 1, 0)): a part whose required parts are not
            // on stays off, and the parts it stands in for come off.
            let mut fitted = std::collections::BTreeSet::new();
            for level in 1..=self.perf_level {
                for category in 0..=9u8 {
                    for p in packages.iter().filter(|p| p.category == category && p.level as i32 == level) {
                        for part in &p.parts {
                            crate::performance::install(&mut fitted, packages, part.id);
                        }
                    }
                }
            }
            fitted
        } else {
            self.perf_parts.iter().enumerate().filter(|&(_, &f)| f != 0xFF && f != 0).map(|(id, _)| id as u8).collect()
        }
    }
}

pub fn parse_presets(globalb: &[u8]) -> Vec<Preset> {
    bundle::chunks(globalb, 0)
        .find(|c| c.id == 0x30220)
        .map(|c| {
            // The records start at the chunk's body, unaligned.
            c.body
                .chunks_exact(0x338)
                .map(|r| Preset {
                    name: read_cstr(r, 0x28, 0x20),
                    car: read_cstr(r, 0x08, 0x20),
                    perf_level: read_u32(r, 0x48) as i32,
                    parts: (0..0xAA).map(|i| read_u32(r, 0x4C + i * 4)).collect(),
                    perf_parts: r[0x2F4..0x2F4 + 0x44].to_vec(),
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::parse_car_types;
    use crate::formats::parts::{Part, PartDb, SLOT_TYPES};

    /// The indexed searches return what a scan of every part does, for
    /// every car type (and the shared groups' names) in every slot.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn indexed_search_matches_scan() {
        let root = crate::testkit::game_root();
        let g = std::fs::read(root.join("GLOBAL/GLOBALB.BUN")).unwrap();
        let db = PartDb::load(&g).unwrap();
        let mut cars: Vec<String> = parse_car_types(&g).unwrap().into_iter().map(|t| t.name).collect();
        cars.push(String::new());
        assert!(cars.len() > 30);
        let ptrs = |v: Vec<&Part>| v.into_iter().map(|p| p as *const Part).collect::<Vec<_>>();
        for car in &cars {
            for slot in 0..SLOT_TYPES.len() + 1 {
                let scan = db.slot_parts_scan(car, slot);
                assert_eq!(ptrs(db.slot_parts(car, slot)), ptrs(scan.clone()), "{car} slot {slot:#x}");
                let first = |level: Option<u8>| scan.iter().copied().find(|p| level.is_none_or(|l| p.level == l)).map(|p| p as *const Part);
                assert_eq!(db.find(car, slot, None, Some(0)).map(|p| p as *const Part), first(Some(0)), "{car} slot {slot:#x}");
                assert_eq!(db.find(car, slot, None, None).map(|p| p as *const Part), first(None), "{car} slot {slot:#x}");
                if let Some(p) = scan.last() {
                    assert_eq!(db.find(car, slot, Some(p.hash), None).map(|p| p.hash), Some(p.hash));
                }
            }
        }
    }
}
