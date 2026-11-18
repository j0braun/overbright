//! The car part database (GLOBALB chunk 0x80034602), as the loader at
//! 0x636CDB lays it out. Its children follow each other in this order and
//! are read from right after their 8-byte headers (not 16-byte aligned):
//!
//! | child   | what |
//! |---------|------|
//! | 0x34603 | header: per table a pointer slot and a count (0x3C bytes) |
//! | 0x34606 | string pool, dword aligned; model names index it by dword |
//! | 0x3460C | attribute lists: `i16 count`, then `count` u16 attribute indices |
//! | 0x34605 | attributes, 8 bytes: `u32 key hash, u32 value` |
//! | 0x3460A | model records, 0x24 bytes (see [`Model`]) |
//! | 0x3460B | u32 group hashes: a car's name or a shared group; part +7 indexes it |
//! | 0x34604 | parts, 0x0E bytes (see [`Part`]) |

use anyhow::{Context, Result, bail};

use super::bundle::Chunk;
use super::reader::{FormatResult, Reader};
use super::hash::{bin_hash, bin_hash_from};

/// The part type each of the car's 0xAA slots takes (the EXE's table at
/// 0x802960): part +0x04 must equal it. Several slots share a type (the
/// front and rear brakes 0x1B, the wheels 0x1C, …).
pub static SLOT_TYPES: crate::exe_tables::Table<[u8; 0xAA]> = crate::exe_tables::Table::new(|t| &t.slot_types);

/// Each slot's cost category (set up at 0x501920): the record of the
/// cost table (career chunk 0x34A1C) that prices it; 0 none. Paint and
/// the slots 0x40–0x43 have their own rules ([`PartDb::category`]).
pub static SLOT_CATEGORIES: crate::exe_tables::Table<[u8; 0xAA]> = crate::exe_tables::Table::new(|t| &t.slot_categories);

/// Group hashes whose cars skip parts marked EXCLUDE_UG1 (0x610030) and
/// EXCLUDE_UG2 (0x6100B0), and the SUV groups that skip EXCLUDE_SUV
/// (0x61B680).
const EXCLUDE_UG1_CARS: [u32; 15] = [
    0x0345_5D6B, 0x0016_3087, 0x0000_DB7C, 0x0000_DC00, 0x000A_C6D1, 0x0150_FB80, 0x0290_C04D, 0x02CA_04DF, 0x72B2_E513, 0x03B8_C48A, 0x04FD_0522,
    0x545A_EC60, 0x7502_DA84, 0x79AE_BFEC, 0xBE48_375E,
];
const EXCLUDE_UG2_CARS: [u32; 13] =
    [0x0302_36EE, 0x0000_ACC9, 0x0000_0453, 0x0000_06E7, 0x0000_A86E, 0x0000_DC01, 0x0293_FAD7, 0x4B10_070A, 0x09D2_EFF6, 0x2D7C_9B5D, 0x3516_5819, 0xCF44_58F8, 0xF77C_13EB];
const SUV_GROUPS: [u32; 3] = [0x0219_C94A, 0x6126_DD4D, 0xCF70_4771];

/// A part (0x34604, 0x0E bytes).
#[derive(Clone, Copy, Debug)]
pub struct Part {
    /// +0x00: the part's hash.
    pub hash: u32,
    /// +0x04: the slot it fills (0..0xAA; 0x610300 reads the car's part
    /// per slot).
    pub slot: u8,
    /// +0x05: low 5 bits a kind within the slot, top 3 bits the upgrade
    /// level (0 stock; visual points by level, 0x5142E0).
    pub kind: u8,
    pub level: u8,
    /// +0x07: its group, an index into the group hashes (0x3460B).
    pub group: u8,
    /// +0x06: what its model names continue from ([`PartDb::model_seed`]).
    pub flag: u8,
    /// +0x08: its name in the string pool, by dword ("STOCK", "STOCK
    /// RIMS", "BASE KIT00").
    pub name: u16,
    /// +0x0A: offset in u16s into the attribute lists (0xFFFF none;
    /// 0x60FF00).
    pub attributes: u16,
    /// +0x0C: index into the model records (0xFFFF none).
    pub model: u16,
}

/// A model record (0x3460A, 0x24 bytes): +0x00 non-zero when the names
/// are string-pool indices (the loader turns them into pointers),
/// otherwise solid hashes; +0x02 the name's middle part ("_KIT00",
/// "_BASE", "_DECAL", "_WIDE1"; 0xFFFF none); then two groups of four
/// names, one per LOD (A–D), 0xFFFFFFFF for none. A named model's solid
/// is `<seed><middle><name>_<LOD letter>` (240SX_KIT00_FRONT_BUMPER_A; the
/// seed is the car for its own parts, see [`PartDb::model_seed`]);
/// verified against every player car's GEOMETRY.BIN by
/// the private developer probe `stock_check`.
#[derive(Clone, Copy, Debug)]
pub struct Model {
    pub named: bool,
    pub middle: u16,
    pub names: [[u32; 4]; 2],
}

#[derive(Clone)]
pub struct PartDb {
    pub parts: Vec<Part>,
    pub models: Vec<Model>,
    /// 0x34605: (key hash, value).
    pub attributes: Vec<(u32, u32)>,
    /// 0x3460C as u16s.
    pub lists: Vec<u16>,
    /// 0x34606.
    pub strings: Vec<u8>,
    /// 0x3460B: the group hashes.
    pub groups: Vec<u32>,
    /// GLOBALB 0x34607: per slot the two groups a car's parts come from
    /// (0xFFFFFFFF the car's own name hash), then overrides {car hash,
    /// slot, two groups} (0x6101F0).
    pub slot_groups: Vec<[u32; 2]>,
    pub group_overrides: Vec<(u32, u32, [u32; 2])>,
    /// Built on first use: per (group index, part type) the indices of the
    /// parts a search for that group may return (in database order, the
    /// group's exclusions applied), so a slot's search no longer scans
    /// every part (a performance audit finding).
    index: std::sync::OnceLock<std::collections::HashMap<(u8, u8), Vec<u32>>>,
}

impl PartDb {
    /// Parse the 0x80034602 chunk.
    pub fn parse(db: Chunk) -> Result<PartDb> {
        let mut parts = Vec::new();
        let mut models = Vec::new();
        let mut attributes = Vec::new();
        let mut lists = Vec::new();
        let mut strings = Vec::new();
        let mut groups = Vec::new();
        for c in db.children() {
            let b = Reader::new(c.body);
            match c.id {
                0x34604 => {
                    parts = b.map_records(0x0E, |r| {
                        let kind = r.u8_at(5)?;
                        Ok(Part {
                            hash: r.u32_at(0)?,
                            slot: r.u8_at(4)?,
                            kind: kind & 0x1F,
                            level: kind >> 5,
                            group: r.u8_at(7)?,
                            flag: r.u8_at(6)?,
                            name: r.u16_at(8)?,
                            attributes: r.u16_at(0x0A)?,
                            model: r.u16_at(0x0C)?,
                        })
                    })?
                }
                0x3460A => {
                    models = b.map_records(0x24, |r| {
                        let mut names = [[0u32; 4]; 2];
                        for (g, row) in names.iter_mut().enumerate() {
                            for (i, v) in row.iter_mut().enumerate() {
                                *v = r.u32_at(4 + g * 16 + i * 4)?;
                            }
                        }
                        Ok(Model { named: r.u8_at(0)? != 0, middle: r.u16_at(2)?, names })
                    })?
                }
                0x34605 => attributes = b.map_records(8, |r| Ok((r.u32_at(0)?, r.u32_at(4)?)))?,
                0x3460C => lists = b.map_records(2, |r| r.u16_at(0))?,
                0x34606 => strings = c.body.to_vec(),
                0x3460B => groups = b.map_records(4, |r| r.u32_at(0))?,
                _ => {}
            }
        }
        if parts.is_empty() {
            bail!("part database without parts");
        }
        Ok(PartDb { parts, models, attributes, lists, strings, groups, slot_groups: Vec::new(), group_overrides: Vec::new(), index: std::sync::OnceLock::new() })
    }

    /// Find the database and the slot groups (0x34607) in GLOBALB.BUN and
    /// parse them.
    pub fn load(globalb: &[u8]) -> Result<PartDb> {
        let db = super::bundle::chunks(globalb, 0).find(|c| c.id == 0x8003_4602).context("no part database")?;
        let mut db = PartDb::parse(db)?;
        let slots = super::bundle::chunks(globalb, 0).find(|c| c.id == 0x34607).context("no slot groups")?;
        let (defaults, overrides) = slots.body.split_at(0x550.min(slots.body.len()));
        db.slot_groups = Reader::new(defaults).map_records(8, |r| Ok([r.u32_at(0)?, r.u32_at(4)?]))?;
        db.group_overrides = Reader::new(overrides).map_records(16, |r| Ok((r.u32_at(0)?, r.u32_at(4)?, [r.u32_at(8)?, r.u32_at(12)?])))?;
        Ok(db)
    }

    /// The two groups a car's parts for `slot` come from (0x6101F0).
    pub fn slot_groups(&self, car: &str, slot: usize) -> [u32; 2] {
        let car_hash = bin_hash(car);
        if let Some((_, _, g)) = self.group_overrides.iter().find(|(c, s, _)| *c == car_hash && *s as usize == slot) {
            return *g;
        }
        let g = self.slot_groups.get(slot).copied().unwrap_or([0xFFFF_FFFF; 2]);
        g.map(|h| if h == 0xFFFF_FFFF { car_hash } else { h })
    }

    /// The parts of group `group` (a hash) of part type `kind` the group's
    /// cars may take, in database order: what a scan of every part with
    /// `p.group` = the group's first index, `p.slot == kind` and not
    /// [`Self::excluded`] yields. Exclusions depend only on the group
    /// hash, which the part's group index gives.
    fn group_parts(&self, group: u32, kind: u8) -> impl Iterator<Item = &Part> + '_ {
        let index = self.index.get_or_init(|| {
            let mut m: std::collections::HashMap<(u8, u8), Vec<u32>> = std::collections::HashMap::new();
            for (i, p) in self.parts.iter().enumerate() {
                let Some(&g) = self.groups.get(p.group as usize) else { continue };
                if !self.excluded(p, g) {
                    m.entry((p.group, p.slot)).or_default().push(i as u32);
                }
            }
            m
        });
        let list = self
            .groups
            .iter()
            .position(|&h| h == group)
            .filter(|&i| i <= u8::MAX as usize)
            .and_then(|i| index.get(&(i as u8, kind)))
            .map_or(&[][..], |v| &v[..]);
        list.iter().map(move |&i| &self.parts[i as usize])
    }

    /// The first part for `slot` on `car` (0x61B990 → 0x61B680): from the
    /// slot's groups in turn, of the slot's part type, with the given hash
    /// and level when asked, skipping parts the car's group excludes.
    pub fn find(&self, car: &str, slot: usize, hash: Option<u32>, level: Option<u8>) -> Option<&Part> {
        let kind = *SLOT_TYPES.get(slot)?;
        self.slot_groups(car, slot)
            .into_iter()
            .filter(|&g| g != 0)
            .find_map(|group| self.group_parts(group, kind).find(|p| hash.is_none_or(|h| p.hash == h) && level.is_none_or(|l| p.level == l)))
    }

    /// A part the group's cars skip (0x61B680): EXCLUDE_UG1 / EXCLUDE_UG2
    /// on the listed cars, EXCLUDE_SUV on the SUV groups.
    fn excluded(&self, p: &Part, group: u32) -> bool {
        let flag = |name: &str| self.attribute(p, bin_hash(name)).is_some_and(|v| v != 0);
        (EXCLUDE_UG1_CARS.contains(&group) && flag("EXCLUDE_UG1"))
            || (EXCLUDE_UG2_CARS.contains(&group) && flag("EXCLUDE_UG2"))
            || (SUV_GROUPS.contains(&group) && flag("EXCLUDE_SUV"))
    }

    /// Every part for `slot` on `car`, any level, in the order repeated
    /// [`PartDb::find_after`] calls walk them (one pass instead of one
    /// search from the start per part).
    pub fn parts_for(&self, car: &str, slot: usize) -> Vec<&Part> {
        let Some(&kind) = SLOT_TYPES.get(slot) else { return Vec::new() };
        let mut out: Vec<&Part> = Vec::new();
        for group in self.slot_groups(car, slot).into_iter().filter(|&g| g != 0) {
            for p in self.group_parts(group, kind) {
                // find_after resumes after a part's first place in the
                // walk, so a part met again ends it.
                if out.iter().any(|q| std::ptr::eq(*q, p)) {
                    return out;
                }
                out.push(p);
            }
        }
        out
    }

    /// The part after `after` for `slot` on `car`, any level (0x61BA40 /
    /// 0x61BA60 walk them in the order 0x61B990 searches).
    pub fn find_after(&self, car: &str, slot: usize, after: Option<&Part>) -> Option<&Part> {
        let kind = *SLOT_TYPES.get(slot)?;
        let mut candidates = self.slot_groups(car, slot).into_iter().filter(|&g| g != 0).flat_map(|group| self.group_parts(group, kind));
        match after {
            None => candidates.next(),
            Some(a) => candidates.skip_while(|p| !std::ptr::eq(*p, a)).nth(1),
        }
    }

    /// Every part for `slot` on `car`, any level, in the order 0x61B990
    /// walks them (the slot's first group, then its second; what
    /// [`Self::find_after`] steps through one at a time).
    pub fn slot_parts(&self, car: &str, slot: usize) -> Vec<&Part> {
        let Some(&kind) = SLOT_TYPES.get(slot) else { return Vec::new() };
        self.slot_groups(car, slot).into_iter().filter(|&g| g != 0).flat_map(|group| self.group_parts(group, kind)).collect()
    }

    /// [`Self::slot_parts`] by a scan of every part (the indexed search's
    /// reference).
    #[cfg(test)]
    fn slot_parts_scan(&self, car: &str, slot: usize) -> Vec<&Part> {
        let Some(&kind) = SLOT_TYPES.get(slot) else { return Vec::new() };
        self.slot_groups(car, slot)
            .into_iter()
            .filter(|&g| g != 0)
            .flat_map(|group| {
                let index = self.groups.iter().position(|&h| h == group);
                self.parts.iter().filter(move |p| Some(p.group as usize) == index && p.slot == kind && !self.excluded(p, group))
            })
            .collect()
    }

    /// The stock part for `slot` (0x637050 through 0x61BC90): the first of
    /// level 0.
    pub fn stock(&self, car: &str, slot: usize) -> Option<&Part> {
        self.find(car, slot, None, Some(0))
    }

    /// A part's attributes (0x60FF00).
    pub fn attributes(&self, part: &Part) -> Vec<(u32, u32)> {
        if part.attributes == 0xFFFF {
            return Vec::new();
        }
        let at = part.attributes as usize;
        let Some(&count) = self.lists.get(at) else { return Vec::new() };
        (0..count as usize).filter_map(|i| self.lists.get(at + 1 + i)).filter_map(|&a| self.attributes.get(a as usize).copied()).collect()
    }

    /// One attribute of a part (0x61B510), by key hash.
    pub fn attribute(&self, part: &Part, key: u32) -> Option<u32> {
        // As `attributes(part)`'s first with the key, without the list.
        if part.attributes == 0xFFFF {
            return None;
        }
        let at = part.attributes as usize;
        let count = *self.lists.get(at)? as usize;
        (0..count).filter_map(|i| self.lists.get(at + 1 + i)).filter_map(|&a| self.attributes.get(a as usize)).find(|&&(k, _)| k == key).map(|&(_, v)| v)
    }

    /// A string-pool string at dword index `index`.
    pub fn string(&self, index: u32) -> Option<String> {
        let at = index as usize * 4;
        let rest = self.strings.get(at..)?;
        let end = rest.iter().position(|&b| b == 0)?;
        // Single-byte text (Windows-1252: "BURGER KING\xAE", "EDGE\xAE
        // ADVANCED\x99 SHAVE GEL"); the original shows ® and ™ in the
        // vinyl names. UTF-8 decoding dropped them.
        Some(rest[..end].iter().map(|&b| cp1252(b)).collect())
    }

    /// The hash a named model's solid names continue from (0x634914 in
    /// the car model builder 0x634810, and 0x633660): by part +0x06, 0 none
    /// (the plain hash start 0xFFFFFFFF), 1 the part's group hash (a car's
    /// own parts: its name), otherwise the part's brand, attribute
    /// 0xEBB03E66 (0x4FE110 → 0x61B510, 0 without one) — the shared rims
    /// (`ENKEI` + `_STYLE01` + `_16_24` + `_A`).
    pub fn model_seed(&self, part: &Part) -> u32 {
        match part.flag {
            0 => 0xFFFF_FFFF,
            1 => self.groups.get(part.group as usize).copied().unwrap_or(0),
            _ => self.attribute(part, 0xEBB0_3E66).unwrap_or(0),
        }
    }

    /// The solid hashes of a part's model: per group (a part may draw two,
    /// e.g. the left and right mirrors), per LOD from A (0x634810: the
    /// seed, then the middle part, the name and `_<LOD letter>`; a hashed
    /// model's name is the solid hash itself; a name of 0xFFFFFFFF ends
    /// the group).
    pub fn solids(&self, part: &Part, _car: &str) -> Vec<Vec<u32>> {
        let Some(m) = self.models.get(part.model as usize) else { return Vec::new() };
        let seed = self.model_seed(part);
        let middle = if m.middle == 0xFFFF { String::new() } else { self.string(m.middle as u32).unwrap_or_default() };
        m.names
            .iter()
            .map(|group| {
                group
                    .iter()
                    .enumerate()
                    .take_while(|&(_, &n)| n != 0xFFFF_FFFF)
                    .map(|(lod, &n)| {
                        if m.named {
                            let name = self.string(n).unwrap_or_default();
                            bin_hash_from(seed, &format!("{middle}{name}_{}", (b'A' + lod as u8) as char))
                        } else {
                            n
                        }
                    })
                    .collect::<Vec<u32>>()
            })
            .filter(|g| !g.is_empty())
            .collect()
    }

    /// The model names of a part (group 0, LOD A first), as text when the
    /// record holds strings.
    pub fn model_names(&self, part: &Part) -> Vec<String> {
        let Some(m) = self.models.get(part.model as usize) else { return Vec::new() };
        m.names
            .iter()
            .flatten()
            .filter(|&&n| n != 0xFFFF_FFFF)
            .map(|&n| if m.named { self.string(n).unwrap_or_else(|| format!("#{n}")) } else { format!("{n:08X}") })
            .collect()
    }
}

/// One level of a cost category (12 bytes in a 0x34A1C record).
#[derive(Clone, Copy, Debug, Default)]
pub struct CostLevel {
    /// +0: visual points (0x501860).
    pub points: i16,
    /// +2: price (0x514280; −1 none).
    pub price: i16,
    /// +4: what unlocks it (0x515360): 1 an event won (+8 its hash),
    /// 2 normal, 3 URL, 4 sponsor wins in the stage (+8 & 0xFFFF).
    pub unlock: u8,
    /// +5: the level the unlock test is asked for (0x515470 passes this
    /// signed byte to 0x515360).
    pub level: i8,
    pub unlock_value: u32,
}

/// The part cost table (career chunk 0x34A1C, 0x28-byte records: +0 the
/// category, then levels 1–3 at +4, +0x10, +0x1C; 0x501750).
#[derive(Clone, Debug, Default)]
pub struct CostTable {
    pub categories: Vec<[CostLevel; 3]>,
}

impl CostTable {
    pub fn parse(body: &[u8]) -> CostTable {
        let level = |r: &Reader<'_>, l: usize| -> FormatResult<CostLevel> {
            let at = 4 + l * 12;
            Ok(CostLevel { points: r.i16_at(at)?, price: r.i16_at(at + 2)?, unlock: r.u8_at(at + 4)?, level: r.u8_at(at + 5)? as i8, unlock_value: r.u32_at(at + 8)? })
        };
        // Whole 0x28-byte records: no read fails.
        let categories = Reader::new(body).map_records(0x28, |r| Ok([level(&r, 0)?, level(&r, 1)?, level(&r, 2)?])).unwrap_or_default();
        CostTable { categories }
    }

    fn level(&self, category: u8, level: u8) -> Option<&CostLevel> {
        if !(1..=0x56).contains(&category) || !(1..=3).contains(&level) {
            return None;
        }
        self.categories.get(category as usize - 1).map(|c| &c[level as usize - 1])
    }

    /// Visual points of a category at a level (0x501860): level 4 parts
    /// have fixed points.
    pub fn points(&self, category: u8, level: u8) -> i32 {
        if level == 4 {
            return match category {
                0x0B | 0x0C | 0x0D | 0x22 | 0x4A | 0x4B => 400,
                0x16 => 1000,
                0x3E => 100,
                _ => 0,
            };
        }
        self.level(category, level).map_or(0, |l| l.points as i32)
    }

    /// Price of a category at a level (0x514280): level 0 parts cost
    /// nothing except category 0x4C, priced as level 1.
    pub fn price(&self, category: u8, level: u8) -> i32 {
        let level = match level {
            4 => return 0,
            0 if category == 0x4C => 1,
            0 => return 0,
            l => l,
        };
        self.level(category, level).map_or(0, |l| if l.price == -1 { 0 } else { l.price as i32 })
    }
}

impl PartDb {
    /// A part's cost category in `slot` (0x501D70).
    pub fn category(&self, slot: usize, part: &Part) -> u8 {
        let brand = || self.attribute(part, 0xEBB0_3E66);
        if slot == 0x3F {
            return match brand() {
                Some(h) if h == bin_hash("METAL") => 0x26,
                Some(h) if h == bin_hash("GLOSS") => 0x25,
                Some(h) if h == bin_hash("PEARL") => 0x27,
                Some(h) if h == bin_hash("MUFFLER") => 0x4E,
                Some(h) if h == bin_hash("RIM") => 0x4D,
                Some(h) if h == bin_hash("HOSE") => 0x28,
                _ => 0x26,
            };
        }
        if (0x40..=0x43).contains(&slot) {
            static BY_KIND: crate::exe_tables::Table<[u8; 28]> = crate::exe_tables::Table::new(|t| &t.vinyl_categories);
            return BY_KIND.get(part.kind as usize).copied().unwrap_or(0);
        }
        if self.attribute(part, 0x721A_FF7C).is_some_and(|v| v != 0) {
            return match slot {
                7 => 0x46,
                9 => 0x4B,
                0x0C => 0x4A,
                0x20 => 0x45,
                _ => 0,
            };
        }
        if slot == 0x1D && brand() == Some(0xC8D1_A75E) {
            return 0x21;
        }
        if slot == 9 && part.kind == 5 {
            return 0x22;
        }
        SLOT_CATEGORIES.get(slot).copied().unwrap_or(0)
    }

    /// The car's visual rating, 0..10 (0x5234D0): the visual points of the
    /// parts in all slots but the vinyl colours (0x4E–0x5C) and slot 0x1E,
    /// and without the bumpers and skirt (1, 2, 0x0B) under a widebody
    /// (slot 6 above level 0); the stock paint scores nothing (0x5142E0).
    /// Every 500 points is one step (table 0x7FA238), up to 10 at 5000:
    /// see [`rating_from_points`].
    pub fn visual_rating(&self, costs: &CostTable, stock_paint: u32, installed: &dyn Fn(usize) -> Option<Part>) -> f32 {
        let widebody = installed(6).is_some_and(|p| p.level != 0);
        let mut points = 0;
        for slot in 0..0xAA {
            if (0x4E..=0x5C).contains(&slot) || slot == 0x1E || (widebody && matches!(slot, 1 | 2 | 0x0B)) {
                continue;
            }
            let Some(p) = installed(slot) else { continue };
            if slot == 0x3F && p.hash == stock_paint {
                continue;
            }
            points += costs.points(self.category(slot, &p), p.level).max(0);
        }
        rating_from_points(points)
    }
}

/// 0x5234D0's step from points to the rating: the (rating, points) table
/// at 0x7FA238 — (0, 0), (1, 500) … (10, 5000) — interpolated on the x87
/// stack, `k + (points − 500·k) / 500` with `k` the step below, 10 from
/// 5000 points on (0x7FA288 / 0x7FA28C). The FPU runs at 24-bit precision
/// there (Direct3D's default control word), so the division and the add
/// each round to f32: 1680 points give 3.3600001 (3 + 0.36), not
/// f32(3.36) = 3.36 (**observed** under Wine: the original's 0x5234D0 for
/// 1 680, 1 180, 1 320, 1 430, 1 445, 1 695 and 3 985 points;
/// docs/visual-rating.md).
pub fn rating_from_points(points: i32) -> f32 {
    if points >= 5000 {
        return 10.0;
    }
    let k = points.max(0) / 500;
    k as f32 + (points - 500 * k) as f32 / 500.0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The original's 0x5234D0 under Wine (bits read from its result,
    /// docs/visual-rating.md): 24-bit x87 rounding of k + rest / 500.
    #[test]
    fn rating_rounds_as_the_original() {
        for (points, bits) in [(0, 0u32), (1370, 0x402F_5C29), (1680, 0x4057_0A3E), (1445, 0x4038_F5C2), (3985, 0x40FF_0A3E), (4520, 0x4110_A3D7), (500, 0x3F80_0000), (5000, 0x4120_0000), (9000, 0x4120_0000)] {
            assert_eq!(rating_from_points(points).to_bits(), bits, "{points}");
        }
        // f32(points / 500) would round 1680 to 3.36 exactly.
        assert_ne!((1680f32 / 500.0).to_bits(), 0x4057_0A3E);
    }

    /// The indexed searches return what a scan of every part does, for
    /// every car type (and the shared groups' names) in every slot.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn indexed_search_matches_scan() {
        let root = crate::testkit::game_root();
        let g = std::fs::read(root.join("GLOBAL/GLOBALB.BUN")).unwrap();
        let db = PartDb::load(&g).unwrap();
        let mut cars: Vec<String> = crate::car::parse_car_types(&g).unwrap().into_iter().map(|t| t.name).collect();
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

    /// `parts_for` walks the parts as repeated `find_after` calls do.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn parts_for_matches_find_after() {
        let root = crate::testkit::game_root();
        let db = PartDb::load(&std::fs::read(root.join("GLOBAL/GLOBALB.BUN")).unwrap()).unwrap();
        for car in ["240SX", "350Z", "COROLLA", "ESCALADE", "HUMMER", "NAVIGATOR", "GTO", "MIATA", "IMPREZAWRX"] {
            for slot in 0..SLOT_TYPES.len() {
                let mut walk = Vec::new();
                let mut after = None;
                while let Some(p) = db.find_after(car, slot, after) {
                    after = Some(p);
                    walk.push(p as *const Part);
                }
                let one: Vec<*const Part> = db.parts_for(car, slot).into_iter().map(|p| p as *const Part).collect();
                assert_eq!(one, walk, "{car} slot {slot:#x}");
            }
        }
    }
}

/// A Windows-1252 byte as a character (0x80–0x9F hold the typographic
/// signs; the rest is Latin-1).
pub fn cp1252(b: u8) -> char {
    const HIGH: [u16; 32] = [
        0x20AC, 0x81, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039, 0x0152, 0x8D, 0x017D, 0x8F, //
        0x90, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x9D, 0x017E, 0x0178,
    ];
    match b {
        0x80..=0x9F => char::from_u32(HIGH[(b - 0x80) as usize] as u32).unwrap_or(b as char),
        _ => b as char,
    }
}
