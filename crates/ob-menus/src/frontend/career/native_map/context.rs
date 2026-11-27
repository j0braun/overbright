//! What the native save mapping needs from the game data.

use crate::frontend::career::*;

/// The visit table's +6 states (career +0x7B00 entries, **verified**
/// writers): 1 won (0x541430), 2 the current event (a race's start
/// 0x541170 unless won, the intro event 0x5410F0), 3 its race dialog
/// declined (0x5138F0), 4 lost (0x528510 unless won), 5 a shop entered
/// (0x54CB00). A milestone's offer is the entry's +4 = 2 (0x5331D0,
/// 0x5337E0, 0x533BF0), its photo shoot +4 = 2 and +6 = 1 (0x533690); a
/// shop's visit +4 = 1
/// (0x512FF0).
pub mod visit {
    pub const WON: u8 = 1;
    pub const CURRENT: u8 = 2;
    pub const DECLINED: u8 = 3;
    pub const LOST: u8 = 4;
    pub const ENTERED: u8 = 5;
    pub const OFFERED: u8 = 2;
}

/// What the native profile mapping needs from the game data: the events,
/// milestones, shops, world challenges and sponsors by hash, the car types
/// by their bin hash, and each car's stock part per slot.
pub struct NativeContext {
    /// By event index: (hash, stage record, category, SUV).
    pub events: Vec<(u32, u8, Category, bool)>,
    pub event_index: HashMap<u32, usize>,
    pub milestones: std::collections::HashSet<u32>,
    /// Each milestone's stage (+0x25), for the stage record's covers list.
    pub milestone_stage: HashMap<u32, u8>,
    pub shops: std::collections::HashSet<u32>,
    pub challenges: std::collections::HashSet<u32>,
    /// bin_hash(sponsor name) → the sponsor's hash, and back.
    pub sponsor_by_name: HashMap<u32, u32>,
    pub sponsor_name: HashMap<u32, u32>,
    pub cars: HashMap<u32, String>,
    pub stock: HashMap<String, Vec<u32>>,
    /// The game's part database (for the owned-part bits' part order).
    pub db: Option<crate::formats::parts::PartDb>,
    /// Each car type's stock paint (car type +0x84C).
    pub paints: HashMap<String, u32>,
    /// Each stage record's definition number (+0xFD; 0 without one).
    pub stage_numbers: Vec<u8>,
    /// [`Self::owned_bits`] per car type, computed once (the game data does
    /// not change; a performance audit found every in-game commit rebuilt them).
    owned_memo: std::sync::Mutex<HashMap<String, std::sync::Arc<[OwnedBit]>>>,
}

/// One part's owned bit: (slot, part hash, byte offset within the extra
/// block, bit mask); see [`NativeContext::owned_bits`].
pub type OwnedBit = (u8, u32, usize, u8);

/// The four vinyl colours a new car record holds in slots 0x4D…0x5C, in
/// turn (**inferred**: the same in the car lot's and main-menu
/// Customize's records of the fixtures, 240SX and 206; the code that sets
/// them is not traced).
pub const VINYL_COLOURS: [u32; 4] = [0xD056_1E94, 0xD056_1E8D, 0x44D2_8FAE, 0xD056_1E8C];

static NATIVE_CONTEXT: std::sync::OnceLock<NativeContext> = std::sync::OnceLock::new();

impl NativeContext {
    pub fn new(root: &Path, data: &CareerData) -> NativeContext {
        let g = crate::game_fs::read(root, "GLOBAL/GLOBALB.BUN").ok();
        let db = g.as_ref().and_then(|g| crate::formats::parts::PartDb::load(g).ok());
        let types = g.as_ref().and_then(|g| crate::car::parse_car_types(g).ok()).unwrap_or_default();
        NativeContext::build(data, db.as_ref(), &types)
    }

    /// Set once at start-up from the game's own part database.
    pub fn init_with(data: &CareerData, db: &crate::formats::parts::PartDb, types: &[crate::car::CarType]) {
        let _ = NATIVE_CONTEXT.set(NativeContext::build(data, Some(db), types));
    }

    fn build(
        data: &CareerData,
        db: Option<&crate::formats::parts::PartDb>,
        types: &[crate::car::CarType],
    ) -> NativeContext {
        let events = data.events.iter().map(|e| (e.unlock, e.stage, e.category, e.suv)).collect();
        let event_index = data.events.iter().map(|e| (e.unlock, e.index)).collect();
        let sponsor_by_name: HashMap<u32, u32> = data.sponsors.values().map(|s| (bin_hash(&s.name), s.hash)).collect();
        let sponsor_name = sponsor_by_name.iter().map(|(&n, &h)| (h, n)).collect();
        let cars: HashMap<u32, String> = data.makes.keys().map(|n| (bin_hash(n), n.clone())).collect();
        let stock = cars
            .values()
            .map(|n| {
                (
                    n.clone(),
                    (0..crate::formats::native_save::PART_SLOTS)
                        .map(|s| db.and_then(|db| db.stock(n, s)).map_or(0, |p| p.hash))
                        .collect(),
                )
            })
            .collect();
        NativeContext {
            events,
            event_index,
            milestones: data.milestones.iter().map(|m| m.hash).collect(),
            milestone_stage: data.milestones.iter().map(|m| (m.hash, m.stage)).collect(),
            shops: data.shops.iter().map(|s| s.hash).collect(),
            challenges: data.mag_triggers.iter().map(|t| t.name).collect(),
            sponsor_by_name,
            sponsor_name,
            cars,
            stock,
            db: db.cloned(),
            paints: types.iter().map(|t| (t.name.clone(), t.stock_paint)).collect(),
            stage_numbers: (0..crate::formats::native_save::STAGE_COUNT)
                .map(|k| data.stages.iter().find(|s| s.stage as usize == k).map_or(0, |s| s.stage))
                .collect(),
            owned_memo: Default::default(),
        }
    }

    /// Set once at start-up (the game data does not change).
    pub fn init(root: &Path, data: &CareerData) {
        let _ = NATIVE_CONTEXT.set(NativeContext::new(root, data));
    }

    pub fn get() -> Option<&'static NativeContext> {
        NATIVE_CONTEXT.get()
    }

    /// Whether a profile may be written with this context: not when the
    /// car types failed to load (an empty table), since every career car
    /// would then be unknown to the mapping. `Profile::store` refuses
    /// with this error (shown as the save's failure).
    pub fn check_writable(&self) -> Result<()> {
        anyhow::ensure!(
            !self.cars.is_empty(),
            "the car types (GLOBAL/GlobalB.lzc) did not load; the profile is not written so that its cars are kept"
        );
        Ok(())
    }

    pub(super) fn stock_part(&self, car: &str, slot: usize) -> u32 {
        self.stock.get(car).and_then(|v| v.get(slot)).copied().unwrap_or(0)
    }

    /// What a new car record holds in a slot nothing was fitted to: the
    /// stock part, else in slot 0x3F the car's stock paint and in
    /// 0x4D…0x5C the vinyl colours (as the original's new records hold).
    pub(super) fn new_record_part(&self, car: &str, slot: usize) -> u32 {
        match self.stock_part(car, slot) {
            0 if slot == 0x3F => self.paints.get(car).copied().unwrap_or(0),
            0 if (0x4D..=0x5C).contains(&slot) => VINYL_COLOURS[(slot - 0x4D) % 4],
            h => h,
        }
    }

    /// The owned-part bits of a career car (**verified** by code; the
    /// bit sets live in the slot's extra block): per part the car can take
    /// in each slot, where its bit sits in the saved block — (byte offset
    /// within the extra block, bit). 0x503D50 sets and 0x503DB0 tests a
    /// part's bit, except in slots 0x3F (paint), 0x44…0x47 and 0x49
    /// (never kept). 0x503B10 finds it: the part's position n among its
    /// slot's parts (0x61BA90: how many parts 0x61B990 walks before it,
    /// any hash, any level, for the car type whose name hash is the
    /// part's group, 0x610020 → 0x610140 (+0x840 its index); a part of a
    /// shared group takes index 0x2E, past the 46 car types — its name
    /// matches no group, so only the slot's shared groups count,
    // prov: inferred frontend.career.owned_bits
    /// **inferred**); then by slot (table 0x503CAC → 0x503C90): 7 →
    /// X+0x26C, 9 → X+0x294, 0xC → X+0x27C, 0x1D / 0x1E → X+0x16C, 0x20 →
    /// X+0x28C, 0xA2 → X+0x29C, each + 4·(n >> 5) with bit n & 31; any
    /// other slot X + 4·(the part's cost category, 0x501D70) with bit
    /// n & 31. X is the slot's memory block +0x434, saved remapped by
    /// 0x503F00 (`OWNED_REMAP`).
    pub fn owned_bits(&self, car: &str) -> std::sync::Arc<[OwnedBit]> {
        if let Some(b) = self.owned_memo.lock().unwrap_or_else(|e| e.into_inner()).get(car) {
            return b.clone();
        }
        let b: std::sync::Arc<[OwnedBit]> = self.owned_bits_fresh(car).into();
        self.owned_memo.lock().unwrap_or_else(|e| e.into_inner()).entry(car.to_string()).or_insert(b).clone()
    }

    /// Whether [`Self::owned_bits`] of `car` is computed (the next call
    /// only reads it).
    pub fn owned_bits_ready(&self, car: &str) -> bool {
        self.owned_memo.lock().unwrap_or_else(|e| e.into_inner()).contains_key(car)
    }

    /// [`Self::owned_bits`] computed from the part database.
    pub fn owned_bits_fresh(&self, car: &str) -> Vec<OwnedBit> {
        let Some(db) = &self.db else {
            return Vec::new();
        };
        // Per (owner, slot): the owner's parts' first places by key, and
        // how many there are (a key not among them takes that count).
        let mut order: HashMap<(&str, usize), (HashMap<u32, usize>, usize)> = HashMap::new();
        let mut out = Vec::new();
        for slot in 0..crate::formats::native_save::PART_SLOTS {
            if matches!(slot, 0x3F | 0x44..=0x47 | 0x49) {
                continue;
            }
            for p in db.slot_parts(car, slot) {
                let group = db.groups.get(p.group as usize).copied().unwrap_or(0);
                let owner = self.cars.get(&group).map_or("", |s| s.as_str());
                let (first, len) = order.entry((owner, slot)).or_insert_with(|| {
                    let list = db.slot_parts(owner, slot);
                    let mut first = HashMap::with_capacity(list.len());
                    for (i, q) in list.iter().enumerate() {
                        first.entry(q.hash ^ (q.group as u32) << 24).or_insert(i);
                    }
                    (first, list.len())
                });
                let key = p.hash ^ (p.group as u32) << 24;
                let n = first.get(&key).copied().unwrap_or(*len);
                let mem = match slot {
                    7 => 0x26C + 4 * (n >> 5),
                    9 => 0x294 + 4 * (n >> 5),
                    0xC => 0x27C + 4 * (n >> 5),
                    0x1D | 0x1E => 0x16C + 4 * (n >> 5),
                    0x20 => 0x28C + 4 * (n >> 5),
                    0xA2 => 0x29C + 4 * (n >> 5),
                    _ => 4 * db.category(slot, p) as usize,
                };
                let Some(&(m, _, f)) =
                    crate::formats::native_save::OWNED_REMAP.iter().find(|&&(m, len, _)| (m..m + len).contains(&mem))
                else {
                    continue;
                };
                let bit = (n & 31) as u32;
                out.push((slot as u8, p.hash, f + mem - m + (bit as usize >> 3), 1u8 << (bit & 7)));
            }
        }
        out
    }
}

/// bin_hash("CAREER_SLOT_<i>"), the slots' names (0x516480; career +0x8740
/// holds the current one's, 0x500BD0).
pub fn slot_id(i: usize) -> u32 {
    bin_hash(&format!("CAREER_SLOT_{i}"))
}
