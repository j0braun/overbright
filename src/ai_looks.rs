//! How the AI racers' cars get their looks (see docs/formats.md, "AI car
//! looks"): parts, paint, vinyls and their colours, rims and neon drawn
//! from the game's random numbers (0x43C1C0) over the part database.
//!
//! A ride's parts are kept as the clone's `installed` map: slot → part
//! hash over the stock car (0x637050), with hash 0 for a slot the game
//! empties (ride +0x590 + 4·slot = 0).

use std::collections::BTreeMap;

use crate::car::CarType;
use crate::formats::parts::{Part, PartDb};
use crate::opponents::GameRng;

/// One row of the AI parts table 0x802C50 (14 × 0x1C bytes, count
/// 0x802DD8): the slot (+0x00), whether a miss keeps the stock part
/// (+0x04, else the slot empties), and per level row (+0x0C + 4·row) the
/// chances (of 100) of a part of level 0, 1, 2, 3.
pub struct SlotChance {
    pub slot: u8,
    pub stock_on_miss: bool,
    pub rows: [[i8; 4]; 4],
}

const MOST: [[i8; 4]; 4] = [[60, 30, 10, 0], [20, 60, 20, 0], [0, 20, 60, 20], [0, 10, 20, 70]];

/// 0x802C50, in the game's order.
pub const SLOT_CHANCES: [SlotChance; 14] = [
    SlotChance { slot: 0x09, stock_on_miss: true, rows: MOST },
    SlotChance { slot: 0x07, stock_on_miss: true, rows: MOST },
    SlotChance { slot: 0x0C, stock_on_miss: true, rows: MOST },
    SlotChance { slot: 0x10, stock_on_miss: true, rows: MOST },
    SlotChance { slot: 0x20, stock_on_miss: true, rows: MOST },
    SlotChance { slot: 0x0F, stock_on_miss: true, rows: MOST },
    SlotChance { slot: 0x0E, stock_on_miss: true, rows: MOST },
    SlotChance { slot: 0x9F, stock_on_miss: true, rows: MOST },
    SlotChance { slot: 0x99, stock_on_miss: false, rows: MOST },
    // Entry 9 (0x802D4C), which 0x632ED0 draws once more.
    SlotChance { slot: 0x3F, stock_on_miss: true, rows: [[0, 70, 30, 0], [0, 70, 20, 10], [0, 20, 60, 20], [0, 10, 20, 70]] },
    SlotChance { slot: 0x49, stock_on_miss: false, rows: [[70, 20, 10, 0], [50, 30, 20, 0], [20, 10, 60, 10], [0, 10, 20, 70]] },
    SlotChance { slot: 0x4A, stock_on_miss: false, rows: [[0, 70, 30, 0], [0, 70, 20, 10], [0, 20, 60, 20], [0, 10, 20, 70]] },
    SlotChance { slot: 0x9A, stock_on_miss: false, rows: [[90, 10, 0, 0], [40, 50, 10, 0], [30, 20, 50, 0], [30, 10, 30, 30]] },
    SlotChance { slot: 0x01, stock_on_miss: false, rows: MOST },
];

/// The rims' level chances by the tyres' level (0x6374C0's table on the
/// stack at 0x63776B, the row category 8's level).
const RIM_ROWS: [[i8; 4]; 4] = [[70, 20, 10, 0], [50, 40, 10, 0], [10, 20, 50, 20], [0, 10, 20, 70]];

/// The vinyl colour sets (0x8A43C8, 5 × 4 hashes the static constructor
/// 0x77F520 fills; count 0x802DE0 = 5).
pub const VINYL_COLOUR_SETS: [[&str; 4]; 5] = [
    ["VINYL_L1_COLOR01", "VINYL_L1_COLOR02", "VINYL_L2_COLOR11", "VINYL_L1_COLOR09"],
    ["VINYL_L1_COLOR01", "VINYL_L1_COLOR02", "VINYL_L1_COLOR03", "VINYL_L1_COLOR04"],
    ["VINYL_L1_COLOR02", "VINYL_L1_COLOR03", "VINYL_L1_COLOR04", "VINYL_L1_COLOR05"],
    ["VINYL_L1_COLOR02", "VINYL_L3_COLOR12", "VINYL_L3_COLOR13", "VINYL_L3_COLOR14"],
    ["VINYL_L1_COLOR02", "VINYL_L3_COLOR18", "VINYL_L3_COLOR19", "VINYL_L3_COLOR20"],
];

/// The paints' matching part paints (0x8A4418, 5 records of 0x28 bytes the
/// static constructor 0x77F2D0 fills; count 0x802DDC = 5): a body paint
/// and the 5 paints slots 0x45 and 0x47 take one of each.
pub const PAINT_MATCHES: [(&str, [&str; 5]); 5] = [
    ("GLOSS_L1_COLOR02", ["GLOSS_L1_COLOR02", "GLOSS_L1_COLOR07", "GLOSS_L2_COLOR02", "GLOSS_L2_COLOR07", "GLOSS_L2_COLOR13"]),
    ("GLOSS_L1_COLOR03", ["GLOSS_L1_COLOR03", "GLOSS_L1_COLOR02", "GLOSS_L2_COLOR01", "GLOSS_L2_COLOR06", "GLOSS_L3_TEST07"]),
    ("GLOSS_L2_COLOR01", ["GLOSS_L2_COLOR01", "GLOSS_L2_COLOR02", "GLOSS_L2_COLOR07", "GLOSS_L2_COLOR11", "GLOSS_L2_COLOR03"]),
    ("GLOSS_L2_COLOR17", ["GLOSS_L2_COLOR17", "GLOSS_L1_COLOR02", "GLOSS_L1_COLOR04", "GLOSS_L2_COLOR01", "GLOSS_L2_COLOR07"]),
    ("METAL_L2_COLOR01", ["METAL_L2_COLOR01", "GLOSS_L1_COLOR02", "GLOSS_L1_COLOR07", "METAL_L2_COLOR05", "METAL_L3_TEST04"]),
];

/// A ride being dressed: the car, its type, the part database and its
/// parts.
pub struct Ride<'a> {
    pub db: &'a PartDb,
    pub car: &'a CarType,
    pub installed: BTreeMap<u8, u32>,
}

impl<'a> Ride<'a> {
    /// A stock ride (0x637050: nothing over the stock parts).
    pub fn stock(db: &'a PartDb, car: &'a CarType) -> Ride<'a> {
        Ride { db, car, installed: BTreeMap::new() }
    }

    /// The car's parts for `slot` in 0x61B990's order, of `level` (None:
    /// any, 0x61B680's −1).
    fn parts(&self, slot: u8, level: Option<u8>) -> Vec<&'a Part> {
        let mut out = self.db.parts_for(&self.car.name, slot as usize);
        out.retain(|p| level.is_none_or(|l| p.level == l));
        out
    }

    /// The first part for `slot` with `hash` (0x61B990 with a hash, any
    /// level).
    fn named(&self, slot: u8, name: &str) -> Option<&'a Part> {
        self.db.find(&self.car.name, slot as usize, Some(crate::formats::hash::bin_hash(name)), None)
    }

    /// Put a part in a slot (ride +0x590 + 4·slot, then 0x61BCE0); the
    /// game never writes slot 0x19 or 0x61–0x68 this way.
    fn set(&mut self, slot: u8, part: Option<&Part>) {
        if slot == 0x19 || (0x61..=0x68).contains(&slot) {
            return;
        }
        self.installed.insert(slot, part.map_or(0, |p| p.hash));
    }

    /// 0x631CC0: a random part of `slot` at `level` (the count 0x631A30,
    /// then Rand_Int(count) + 1 steps of 0x61B990), none when there is
    /// none (no draw).
    pub fn random_part(&self, rng: &mut GameRng, slot: u8, level: Option<u8>) -> Option<&'a Part> {
        let parts = self.parts(slot, level);
        if parts.is_empty() {
            return None;
        }
        Some(parts[rng.below(parts.len() as u32) as usize])
    }

    /// 0x6371F0: a random paint of any level (slot 0x3F), the vinyl slots
    /// 0x40–0x43 emptied; nothing when the car has no paint (no draw).
    pub fn random_paint(&mut self, rng: &mut GameRng) {
        let Some(p) = self.random_part(rng, 0x3F, None) else { return };
        self.set(0x3F, Some(p));
        for slot in 0x40..=0x43 {
            self.set(slot, None);
        }
    }

    /// 0x6371B0(slot, level): a random part of the level in the slot (none
    /// empties it).
    pub fn random_slot(&mut self, rng: &mut GameRng, slot: u8, level: u8) {
        let p = self.random_part(rng, slot, Some(level));
        self.set(slot, p);
    }

    /// 0x61C4C0: a random vinyl (slot 0x40's parts) of `level` (None any)
    /// whose kind (part +5 & 0x1F) is one of `kinds`, up to 512 of them;
    /// None (no draw) when there is none.
    fn random_vinyl(&self, rng: &mut GameRng, level: Option<u8>, kinds: &[u8]) -> Option<&'a Part> {
        let mut found: Vec<&Part> = self.parts(0x40, level).into_iter().filter(|p| kinds.contains(&p.kind)).collect();
        found.truncate(0x200);
        if found.is_empty() {
            return None;
        }
        Some(found[rng.below(found.len() as u32) as usize])
    }

    /// The stock part of a slot (0x61B990(car, slot, 0, 0, 0): the first of
    /// level 0).
    fn stock_part(&self, slot: u8) -> Option<&'a Part> {
        self.db.stock(&self.car.name, slot as usize)
    }

    /// One 0x802C50 row's draw: Rand_Int(100) against the row's running
    /// sums; a level found takes a random part of it (0x631CC0, none
    /// empties the slot), a miss the stock part or nothing.
    fn draw_slot(&mut self, rng: &mut GameRng, e: &SlotChance, row: usize) {
        let k = pick(rng, &e.rows[row.min(3)]);
        let part = if k < 4 {
            self.random_part(rng, e.slot, Some(k as u8))
        } else if e.stock_on_miss {
            self.stock_part(e.slot)
        } else {
            None
        };
        self.set(e.slot, part);
    }

    /// 0x6399E0(1, 1)'s look half, 0x6374C0(1): the AI racer's parts by its
    /// performance levels `levels` (per category 0–9, 0x5995F0: the upgrade
    /// state's float rounded). Each 0x802C50 row by the mean level
    /// (truncated) — slots 1 and 7 by category 9's — then the body kit's
    /// rear bumper and skirts, the rims by category 8's, then 0x632ED0.
    pub fn dress(&mut self, rng: &mut GameRng, levels: [u8; 10]) {
        let mean = (levels.iter().map(|&l| l as f32).sum::<f32>() * 0.1) as usize;
        for e in &SLOT_CHANCES {
            // The jump table 0x637820 by slot − 1 (0x637830): slots 1, 7
            // and 0x0D take category 9, 0x11 category 6, 0x1B category 3,
            // every other the mean.
            let row = match e.slot {
                0x01 | 0x07 | 0x0D => levels[9] as usize,
                0x11 => levels[6] as usize,
                0x1B => levels[3] as usize,
                _ => mean,
            };
            self.draw_slot(rng, e, row);
        }
        // The body kit (slot 1, ride +0x594): its own rear bumper (slot 2)
        // and skirts (0x0B) by name, `<CAR>_KIT<nn>_REAR_BUMPER` and
        // `_SKIRT` with nn its kind; set only when both are found. No kit:
        // the stock (level 0) kit, rear bumper and skirts.
        let kit = self.installed.get(&0x01).copied().filter(|&h| h != 0).and_then(|h| self.db.find(&self.car.name, 0x01, Some(h), None));
        match kit {
            Some(kit) => {
                let n = kit.kind;
                let bumper = self.named(0x02, &format!("{}_KIT{n:02}_REAR_BUMPER", self.car.name));
                let skirt = self.named(0x0B, &format!("{}_KIT{n:02}_SKIRT", self.car.name));
                if let (Some(b), Some(s)) = (bumper, skirt) {
                    self.set(0x02, Some(b));
                    self.set(0x0B, Some(s));
                }
            }
            None => {
                let (k, b, s) = (self.stock_part(0x01), self.stock_part(0x02), self.stock_part(0x0B));
                self.set(0x01, k);
                self.set(0x02, b);
                self.set(0x0B, s);
            }
        }
        // The rims (0x632DF0): a level by category 8 (the tyres), then a
        // random one of that level's slot 0x1D parts of the car's rim size
        // (attribute 0xCE7D8DB5 = type +0xDC) and inches (0xEB0101E2 from
        // +0xDD to +0xDE), up to 128; both rim slots 0x1D and 0x1E take it,
        // none keeps them.
        let k = pick(rng, &RIM_ROWS[(levels[8] as usize).min(3)]);
        if let Some(rim) = self.random_rim(rng, k as u8) {
            self.set(0x1D, Some(rim));
            self.set(0x1E, Some(rim));
        }
        self.dress_paint(rng, levels);
    }

    /// 0x632DF0(ride, level).
    fn random_rim(&self, rng: &mut GameRng, level: u8) -> Option<&'a Part> {
        // 0x61B510 reads an attribute (0 when missing) as a signed byte.
        let attr = |p: &Part, key: u32| self.db.attribute(p, key).unwrap_or(0) as u8 as i8;
        let mut fits: Vec<&Part> = self
            .parts(0x1D, Some(level))
            .into_iter()
            .filter(|p| {
                let inch = attr(p, 0xEB01_01E2);
                attr(p, 0xCE7D_8DB5) == self.car.rim_size as i8 && inch >= self.car.rim_min as i8 && inch <= self.car.rim_max as i8
            })
            .collect();
        fits.truncate(0x80);
        if fits.is_empty() {
            return None;
        }
        Some(fits[rng.below(fits.len() as u32) as usize])
    }

    /// 0x632ED0: the vinyls and their colours, the paint once more, and
    /// the part paints that go with it.
    fn dress_paint(&mut self, rng: &mut GameRng, levels: [u8; 10]) {
        let level = ((levels.iter().map(|&l| l as f32).sum::<f32>() * 0.1) as usize).min(3);
        // A whole-car vinyl (kinds 0x18, 0x1A, 0x1B, any level) by the
        // chance {0, 0, 5, 10}[level]; found, it alone fills slot 0x40.
        let whole = [0u32, 0, 5, 10][level];
        if rng.below(100) < whole {
            if let Some(v) = self.random_vinyl(rng, None, &[0x18, 0x1A, 0x1B]) {
                for slot in 0x41..=0x43 {
                    self.set(slot, None);
                }
                self.set(0x40, Some(v));
            }
        } else {
            // Three layers: kind 0x17 by {95, 0, 5, 0} … {65, 0, 35, 0};
            // kinds 0–0x0D, 0x10–0x14 by {80, 10, 10, 0} … {40, 20, 20,
            // 20}; kind 0x19 by {95, 0, 0, 5} … {65, 0, 0, 35}.
            const A: [[i8; 4]; 4] = [[95, 0, 5, 0], [90, 0, 10, 0], [80, 0, 20, 0], [65, 0, 35, 0]];
            const B: [[i8; 4]; 4] = [[80, 10, 10, 0], [70, 15, 10, 5], [55, 25, 15, 5], [40, 20, 20, 20]];
            const C: [[i8; 4]; 4] = [[95, 0, 0, 5], [90, 0, 0, 10], [80, 0, 0, 20], [65, 0, 0, 35]];
            const B_KINDS: [u8; 19] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 0x0B, 0x0C, 0x0D, 0x10, 0x11, 0x12, 0x13, 0x14];
            let k = pick(rng, &A[level]);
            let a = self.random_vinyl(rng, Some(k as u8), &[0x17]);
            let k = pick(rng, &B[level]);
            let b = self.random_vinyl(rng, Some(k as u8), &B_KINDS);
            let k = pick(rng, &C[level]);
            let c = self.random_vinyl(rng, Some(k as u8), &[0x19]);
            self.set(0x40, a);
            self.set(0x41, b);
            self.set(0x42, c);
            // Each layer's four colours (slots 0x4D + 4·layer): a random
            // set of 0x8A43C8 in a shuffled order (0x610580 for the first
            // layer, the same shuffle inline for the others: each place
            // swapped with place Rand_Int(4)).
            for layer in 0..3u8 {
                let mut order = [0usize, 1, 2, 3];
                for i in 0..4 {
                    let j = rng.below(4) as usize;
                    order.swap(i, j);
                }
                let set = rng.below(VINYL_COLOUR_SETS.len() as u32) as usize;
                for (n, &o) in order.iter().enumerate() {
                    let slot = 0x4D + 4 * layer + n as u8;
                    let p = self.named(slot, VINYL_COLOUR_SETS[set][o]);
                    self.set(slot, p);
                }
            }
        }
        // The paint again (0x802D4C, entry 9 of 0x802C50) by the mean.
        self.draw_slot(rng, &SLOT_CHANCES[9], level);
        // A paint with matches (0x8A4418) gives slots 0x45 and 0x47 one
        // each (two draws first, then the parts by hash, any level).
        let paint = self.installed.get(&0x3F).copied().unwrap_or(0);
        if paint != 0 {
            for (body, matches) in &PAINT_MATCHES {
                if crate::formats::hash::bin_hash(body) == paint {
                    let i = rng.below(matches.len() as u32) as usize;
                    let j = rng.below(matches.len() as u32) as usize;
                    let (p, q) = (self.named(0x45, matches[i]), self.named(0x47, matches[j]));
                    self.set(0x45, p);
                    self.set(0x47, q);
                }
            }
        }
    }
}

/// Rand_Int(100) against a row's running sums: the first place whose sum
/// passes the draw, 4 when none does.
fn pick(rng: &mut GameRng, row: &[i8; 4]) -> usize {
    let r = rng.below(100) as i32;
    let mut sum = 0;
    for (k, &c) in row.iter().enumerate() {
        sum += c as i32;
        if r < sum {
            return k;
        }
    }
    4
}

/// The performance categories' levels (0–3) for [`Ride::dress`]: the
/// upgrade state's floats rounded (0x5995F0), see
/// [`crate::performance::state_levels`].
pub fn category_levels(packages: &[crate::performance::Package], fitted: &dyn Fn(u8) -> bool) -> [u8; 10] {
    crate::performance::state_levels(packages, fitted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_sum_to_100() {
        for e in &SLOT_CHANCES {
            for r in e.rows {
                assert_eq!(r.iter().map(|&c| c as i32).sum::<i32>(), 100, "slot {:#x}", e.slot);
            }
        }
        for r in RIM_ROWS {
            assert_eq!(r.iter().map(|&c| c as i32).sum::<i32>(), 100);
        }
    }

    #[test]
    fn pick_takes_the_first_running_sum_past_the_draw() {
        // State 0x12345678 % 100 = 0x12345678 % 100.
        let mut r = GameRng::default();
        let d = (0x1234_5678u32 % 100) as i32;
        let k = pick(&mut r, &[60, 30, 10, 0]);
        assert_eq!(k, if d < 60 { 0 } else if d < 90 { 1 } else { 2 });
    }
}
