//! Shops and part unlocks.

use super::*;

/// The unlock-all byte 0x838464: when set, 0x52A020 / 0x52A0A0 / 0x52A120
/// / 0x52A1B0 (part, level and category locks) and the car lists
/// (0x5022C0, 0x52FFFF, 0x534121, 0x53B51E) take everything as unlocked.
/// Nothing in SPEED2.EXE writes it (a debug byte); the clone sets it only
/// for comparison shots (`NFSU2_UNLOCK_ALL=1`), to match an original
/// whose byte was poked.
pub static UNLOCK_ALL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

impl Profile {
    /// A part bought for the career car (0x503D50): its owned bit set.
    /// 0x503B10 gives every part of the car a bit by its position n among
    /// its slot's parts (0x61BA90; a shared group's part takes car index
    /// 0x2E, so n is the count of the slot's default groups for all of
    /// them) and `1 << (n & 31)` wraps, so several parts share one bit, and
    /// 0x503DB0 reports them all owned from then on: in the original, one
    /// spoiler of the 206's shared group 0xE8E9C8A4 bought lists every
    /// spoiler of that group (the carbon ones too) in the garage's storage
    /// and opens Carbon Fiber (Wine, the stage-2 save after
    /// JORGEN). So every part sharing the bought part's bit is owned too,
    /// as a save and load of the profile gives (`NativeContext::owned_bits`).
    pub fn own_part(&mut self, slot: u8, hash: u32) {
        self.owned.insert((slot, hash));
        let Some(ctx) = NativeContext::get() else { return };
        let bits = ctx.owned_bits(&self.car);
        if let Some(&(_, _, at, mask)) = bits.iter().find(|b| b.0 == slot && b.1 == hash) {
            self.owned.extend(bits.iter().filter(|b| b.2 == at && b.3 == mask).map(|b| (b.0, b.1)));
        }
    }

    /// Whether a shop is open (0x513110, as 0x533320 tests it too): its
    /// stage is reached, an announced shop (+0x51) has been driven into
    /// (its visit flag, 0x5003B0 → +4 == 1), and its requirement is met
    /// (0x512870: kind +0x9C, value +0x74; 2 / 3 / 4 count the URL,
    /// normal, sponsor wins of the *shop's own* stage record, 0x862018 +
    /// 0x104 × +0x9D, not the current stage's). The debug switch 0x838441
    /// that opens every shop is left off.
    pub fn shop_open(&self, data: &CareerData, shop: &Shop) -> bool {
        if shop.stage > self.stage {
            return false;
        }
        if shop.announced && !self.visited.contains(&shop.hash) {
            return false;
        }
        let (kind, value) = shop.requirement;
        match kind {
            0 => true,
            1 => self.won(data, value),
            2 => value as usize <= self.stage_wins_in(data, shop.stage, Category::Url).len(),
            3 => value as usize <= self.stage_wins_in(data, shop.stage, Category::Normal).len(),
            4 => value as usize <= self.stage_wins_in(data, shop.stage, Category::Sponsor).len(),
            _ => false,
        }
    }

    /// The car enters a shop's trigger (0x512FF0, from 0x5EAFC0 before any
    /// other test): once the shop's stage is reached its visit flag is set.
    /// Returns whether that found an announced shop (+0x51) for the first
    /// time, which the game also puts on the marker table at once
    /// (0x500E20 → 0x4ECCF0).
    pub fn visit_shop(&mut self, shop: &Shop) -> bool {
        if shop.stage > self.stage {
            return false;
        }
        self.touch(shop.hash);
        self.visited.insert(shop.hash) && shop.announced
    }

    /// Whether a part of `category` at `level` can be bought (0x515360):
    /// (the unlock-all byte `UNLOCK_ALL` short-circuits it, as 0x52A1B0 /
    /// 0x52A120 do)
    /// stock parts always, level-4 (unique) parts never, others when the
    /// cost table's unlock (0x501750 → +4 type, +8 value) holds: 1 the
    /// shop named by the value is open, 2 / 3 / 4 the normal, URL, sponsor
    /// wins (0x5004D0 / 0x500520 / 0x5005A0) of stage record value >> 16
    /// (0x862018 + 0x104 × (value >> 16), 0x5153D0…0x51544D; not the
    /// current stage) reach its low half; types 0, 5, 6 open.
    pub fn part_unlocked(&self, data: &CareerData, category: u8, level: u8) -> bool {
        if UNLOCK_ALL.load(std::sync::atomic::Ordering::Relaxed) {
            return true;
        }
        match level {
            0 => return true,
            4 => return false,
            _ => {}
        }
        // The contest vinyls (category 0x44, 0x515370) open with regmebaby
        // (0x579D70), else by the cost table as the rest.
        if category == 0x44 && crate::frontend::cheats::regmebaby() {
            return true;
        }
        if !(1..=0x56).contains(&category) {
            return true;
        }
        let Some(cost) = data.costs.categories.get(category as usize - 1).and_then(|c| c.get(level as usize - 1))
        else {
            return true;
        };
        let count = (cost.unlock_value & 0xFFFF) as usize;
        let stage = (cost.unlock_value >> 16) as u8;
        match cost.unlock {
            1 => data.shops.iter().find(|s| s.hash == cost.unlock_value).is_none_or(|s| self.shop_open(data, s)),
            2 => count <= self.stage_wins_in(data, stage, Category::Normal).len(),
            3 => count <= self.stage_wins_in(data, stage, Category::Url).len(),
            4 => count <= self.stage_wins_in(data, stage, Category::Sponsor).len(),
            _ => true,
        }
    }

    /// Whether a shop lists a part (`hash`) of `category` at `level`
    /// (0x52A020 by 0x50F0D0's flags). In a career (flags 6, 0x515500) the
    /// cost table's unlock (0x515360); for the sponsor vinyls' category
    /// 0x3D the cheat table instead (0x579DB0 by the part's hash: its
    /// code typed at the title, `cheats`). In main-menu Customize
    /// (`scratch`; flag 1, 0x529D70, the Quick Race rule): stock (unlock
    /// level +0x05 >> 5 is 0), or a level a visual code opened (0x579D90:
    /// gimmevisual1 / 2, both level 2), or the cost table's unlock
    /// (0x515360 with mode 4; for 0x3D the vinyl's code, 0x579DB0), or —
    /// once the last stage's (stage record 5, 0x86262C = 0x862018 + 5 ×
    /// 0x104 + 0x100) final event (+0x28) is won (0x500B80) — a level 1–3
    /// part whose unlock is a shop's (0x501790: the cost level's type byte
    /// is 1) or a level-4 one. The unlock-all byte 0x838464 is never set in
    /// play (`UNLOCK_ALL`: shots only). The career state is the loaded
    /// profile's in both (0x515360 reads the global career).
    pub fn part_listed(&self, data: &CareerData, category: u8, level: u8, hash: u32) -> bool {
        if UNLOCK_ALL.load(std::sync::atomic::Ordering::Relaxed) {
            return true;
        }
        let career = if category == 0x3D {
            crate::frontend::cheats::vinyl(hash)
        } else {
            self.part_unlocked(data, category, level)
        };
        if !self.scratch {
            return career;
        }
        let career = career || crate::frontend::cheats::visual(level);
        let final_won = data.stages.get(5).is_some_and(|d| d.final_event != 0 && self.won(data, d.final_event));
        let shop_unlock = || {
            (1..=3).contains(&level)
                && (1..=0x56).contains(&category)
                && data
                    .costs
                    .categories
                    .get(category as usize - 1)
                    .and_then(|c| c.get(level as usize - 1))
                    .is_some_and(|c| c.unlock == 1)
        };
        level == 0 || career || (final_won && (level == 4 || shop_unlock()))
    }

    /// The parts a shop lists for a body slot (0x5449D0 in career shop
    /// mode, 0x515500): every part for the car of the slot's type, carbon
    /// ones apart (attribute 0x721AFF7C), without the type-5 hoods, that
    /// is stock, owned or listed (`part_listed`).
    pub fn shop_parts<'a>(
        &self,
        data: &CareerData,
        db: &'a crate::formats::parts::PartDb,
        slot: usize,
        carbon: bool,
    ) -> Vec<&'a crate::formats::parts::Part> {
        let car = self.car.as_str();
        let mut parts = Vec::new();
        let mut after = None;
        while let Some(p) = db.find_after(car, slot, after) {
            after = Some(p);
            let is_carbon = db.attribute(p, 0x721A_FF7C).is_some_and(|v| v != 0);
            if is_carbon != carbon || (slot == 9 && p.kind == 5) {
                continue;
            }
            let listed = p.level == 0
                || self.owned.contains(&(slot as u8, p.hash))
                || self.part_listed(data, db.category(slot, p), p.level, p.hash);
            if listed {
                parts.push(p);
            }
        }
        parts
    }
}
