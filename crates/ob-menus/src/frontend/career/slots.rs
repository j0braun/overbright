//! Career car slots: the garage, stashing and fitting rides.

use super::*;

/// A career car slot (the player garage's 0x7F8-byte records named
/// CAREER_SLOT_<n>, five made by 0x516480): locked (+0x7F4) until opened
/// (0x5284E0: the first locked one, at a new career and on signing each
/// stage's sponsor), and the stock car the car lot gave it (+0x7F5, set
/// by 0x5348F0 / 0x534960) with that car's parts (+0x434 owned).
#[derive(Clone, Debug, Default)]
pub struct CarSlot {
    pub unlocked: bool,
    pub car: Option<String>,
    pub installed: BTreeMap<u8, u32>,
    pub owned: BTreeSet<(u8, u32)>,
    pub perf: BTreeSet<u8>,
    pub perf_owned: BTreeSet<u8>,
    pub perf_uniques: BTreeSet<u8>,
    pub brands: Vec<u32>,
    pub tuning: [crate::tuning::Settings; TUNING_MODES],
    /// The slot's spend (+0x7F0, [`Profile::spent`]).
    pub spent: u32,
    /// [`Profile::record_levels`].
    pub record_levels: Option<[f32; 10]>,
    /// The loaded file's slot holds a car whose type the native context
    /// does not know (`car` stays `None`): `to_native` keeps the slot's
    /// car flag (+0x7F5) and record as the file has them, so a save never
    /// drops a car the clone did not understand. Cleared when a car is
    /// put in the slot.
    pub opaque: bool,
}

/// The tuning modes of UI_PerformanceTuning_Setting (0x557420): Circuit,
/// Drag, Drift, Street X, URL Race, each its own settings.
pub const TUNING_MODES: usize = 5;

/// Career car slots (0x516480).
pub const CAR_SLOTS: usize = 5;

impl Profile {
    /// Open the first locked car slot (0x5284E0).
    pub fn unlock_slot(&mut self) {
        if let Some(s) = self.slots.iter_mut().find(|s| !s.unlocked) {
            s.unlocked = true;
        }
    }

    /// Keep the current car's state in its slot.
    /// The performance parts bought for every career car slot with its
    /// own car (+0x7F5), the current one included.
    /// Append a bought part's brand to the car's list (0x503E10: the first
    /// empty of 0x44 entries; a 0 leaves it empty).
    pub fn add_brand(&mut self, brand: u32) {
        if brand != 0 && self.brands.len() < 0x44 {
            self.brands.push(brand);
        }
    }

    /// The brands of every career car slot with its own car (0x529E90:
    /// +0x6E0 up to 0x44 entries, 0 ending).
    pub fn slot_brands(&self) -> std::collections::HashSet<u32> {
        let mut out: std::collections::HashSet<u32> =
            if self.car.is_empty() { Default::default() } else { self.brands.iter().copied().collect() };
        for (i, slot) in self.slots.iter().enumerate() {
            if i != self.slot && slot.car.is_some() {
                out.extend(slot.brands.iter().copied());
            }
        }
        out
    }

    pub fn bought_perf_parts(&self) -> std::collections::HashSet<u8> {
        let mut out: std::collections::HashSet<u8> = self.perf_owned.iter().copied().collect();
        for (i, slot) in self.slots.iter().enumerate() {
            if i != self.slot && slot.car.is_some() {
                out.extend(slot.perf_owned.iter().copied());
            }
        }
        out
    }

    pub(super) fn stash(&mut self) {
        if self.slot < self.slots.len() {
            let mut s = std::mem::take(&mut self.slots[self.slot]);
            self.stash_into(&mut s);
            self.slots[self.slot] = s;
        }
    }

    /// The current car's state written into `s` (what [`Profile::stash`]
    /// keeps in the current slot; `unlocked` and `opaque` stay).
    fn stash_into(&self, s: &mut CarSlot) {
        s.car = (!self.car.is_empty()).then(|| self.car.clone());
        s.installed = self.installed.clone();
        s.owned = self.owned.clone();
        s.perf = self.perf.clone();
        s.perf_owned = self.perf_owned.clone();
        s.perf_uniques = self.perf_uniques.clone();
        s.brands = self.brands.clone();
        s.tuning = self.tuning;
        s.spent = self.spent;
        s.record_levels = self.record_levels;
    }

    /// Slot `i` as [`Profile::stash`] would leave it, without changing or
    /// cloning the profile: the current slot built from
    /// the current car's state, every other slot borrowed.
    pub(crate) fn stashed_slot(&self, i: usize) -> Option<std::borrow::Cow<'_, CarSlot>> {
        let s = self.slots.get(i)?;
        if i != self.slot {
            return Some(std::borrow::Cow::Borrowed(s));
        }
        let mut s = s.clone();
        self.stash_into(&mut s);
        Some(std::borrow::Cow::Owned(s))
    }

    /// The cars of the slots a save writes (`to_native`), in slot order.
    pub fn slot_cars(&self) -> Vec<String> {
        (0..self.slots.len().min(5)).filter_map(|i| self.stashed_car(i)).collect()
    }

    /// The car slot `i` names after a stash (`stashed_slot(i).car`, without
    /// copying the parts).
    fn stashed_car(&self, i: usize) -> Option<String> {
        if i == self.slot { (!self.car.is_empty()).then(|| self.car.clone()) } else { self.slots[i].car.clone() }
    }

    pub(super) fn load_slot(&mut self, i: usize) {
        self.slot = i;
        let s = self.slots.get(i).cloned().unwrap_or_default();
        self.car = s.car.unwrap_or_default();
        self.installed = s.installed;
        self.owned = s.owned;
        self.perf = s.perf;
        self.perf_owned = s.perf_owned;
        self.perf_uniques = s.perf_uniques;
        self.brands = s.brands;
        self.tuning = s.tuning;
        self.spent = s.spent;
        self.record_levels = s.record_levels;
    }

    /// A performance shop action's end (Shop_FitPart 0x5213E0, Shop_FitPackage
    /// 0x521350, Return To Stock 0x521A30, the unique 0x521440): the ride is
    /// refreshed by 0x639CF0(1, 1, 1, 1) and stored into the career car's
    /// record by 0x503950 (all 0xAA part slots from the ride, ride +0x590,
    /// and its 0x94-byte upgrade state, ride +0x2F0). 0x639CF0 runs
    /// 0x6399E0 with refit = 1: category 9 (the aerodynamics) refitted to
    /// the mean of the tyres', suspension's and weight's levels
    /// ([`crate::performance::ai_refit`]) as the ride's floats held them
    /// before this action (`before`: the fitted parts the last 0x5B4FD0
    /// rated), then slot 0x0D / 0x1B from the performance parts
    /// ([`crate::car::perf_visual_parts`]) and 0x61BCE0's derived slots
    /// ([`crate::car::ride_slot_parts`]); then 0x5B4FD0 → 0x5AC500 writes
    /// the ten level floats (3 × 0x5A6590 per category). **Verified**
    /// against the original's saves after the same purchases
    /// (`docs/notes/front-end/perf-shop.md`). Slots the lookups leave empty keep their part
    // prov: inferred frontend.career.store_ride
    /// (**inferred** rare: every wide body's lookups resolve).
    pub fn store_ride(
        &mut self,
        db: &crate::formats::parts::PartDb,
        packages: &[crate::performance::Package],
        before: &BTreeSet<u8>,
    ) {
        if self.car.is_empty() {
            return;
        }
        let m = |c: u8| crate::performance::state_float(packages, &|id| before.contains(&id), c) * 0.333_333_34;
        let target = ((m(5) + m(8)) + m(0)) * 0.333_333_34;
        crate::performance::refit(&mut self.perf, packages, target.clamp(0.0, 1.0), &[9]);
        self.store_parts(db, packages);
        let perf = self.perf.clone();
        self.record_levels =
            Some(std::array::from_fn(|c| crate::performance::state_float(packages, &|id| perf.contains(&id), c as u8)));
    }

    /// The ride's part slots as 0x503950 stores them (all 0xAA from the
    /// ride, ride +0x590): slot 0x0D / 0x1B from the performance parts
    /// ([`crate::car::perf_visual_parts`], as the ride's last 0x639CF0 set
    /// them) and 0x61BCE0's derived slots ([`crate::car::ride_slot_parts`],
    /// run by every 0x631D30 fit). Stock parts are kept implicit.
    fn store_parts(&mut self, db: &crate::formats::parts::PartDb, packages: &[crate::performance::Package]) {
        let perf = self.perf.clone();
        let parts = crate::car::perf_visual_parts(&self.installed, db, &self.car, packages, &|id| perf.contains(&id));
        self.installed = crate::car::with_derived_slots(&parts, db, &self.car);
    }

    /// A shop entered (the engage's Enter Shop): the career ride is built
    /// from the car record again (0x516AC0 → 0x503880, 0x639CF0(0, 1, 1,
    /// 1)), so its aerodynamics (category 9) are the body parts' as they
    /// are now (**observed** under Wine: the ESCALADE's ride on entry had
    /// 0x3A off after its front bumper went to stock on the previous
    /// visit; `docs/notes/front-end/shop-record-store.md`). Kept until the shop is left.
    pub fn build_shop_ride(&mut self) {
        self.shop_ride = Some(self.installed.clone());
    }

    /// A visual shop's fit of `hash` in `slot` on the career ride
    /// (0x55C240, and the browsers' own fits by 0x631D30): 0x631D30 sets
    /// the slot (not 0x61–0x68; slot 0x19 only by 0x61BCE0) and slot
    /// 0x1D also takes 0x1E (the rear rims); slot 0x22 (the trunk layout)
    /// refits the components 0x23–0x2E (0x5474B0 → 0x53AEB0,
    /// [`crate::car::trunk_component`]); a hood with CARBONFIBRE
    /// (0x721AFF7C) takes the vinyls of kind 0x14 off the four layers
    /// (0x50EC10; its level ≥ 1 then clears 0x61–0x68 by 0x50EC50, which
    /// 0x631D30 refuses: nothing); slot 0x5D of level ≥ 1 takes off the
    /// vinyls of kinds 0x0E, 0x0F, 0x15–0x1A (0x50EC80) without a wide
    /// body, every vinyl but kind 0x14 (0x50ECE0) with one, and any fit
    /// in slot 6 while such a 0x5D is fitted does the latter — taking the
    /// wide body off (slot 6 back to stock) too: 0x55C316…0x55C335 tests
    /// only the 0x5D part's level (0x610300(0x5D) +5 >> 5), never the new
    /// slot-6 part's (not a bug). **Verified** by code;
    /// `hash` the slot's stock part takes it back to stock.
    pub fn fit_visual(&mut self, db: &crate::formats::parts::PartDb, slot: u8, hash: u32) {
        let car = self.car.clone();
        let set = |installed: &mut BTreeMap<u8, u32>, slot: u8, hash: u32| {
            if hash == 0 || db.stock(&car, slot as usize).is_some_and(|s| s.hash == hash) {
                installed.remove(&slot);
            } else {
                installed.insert(slot, hash);
            }
        };
        if (0x61..=0x68).contains(&slot) || slot == 0x19 {
            return;
        }
        set(&mut self.installed, slot, hash);
        let part = |installed: &BTreeMap<u8, u32>, slot: u8| {
            installed
                .get(&slot)
                .copied()
                .or_else(|| db.stock(&car, slot as usize).map(|p| p.hash))
                .and_then(|h| db.find(&car, slot as usize, Some(h), None))
                .copied()
        };
        let level = |installed: &BTreeMap<u8, u32>, slot: u8| part(installed, slot).map_or(0, |p| p.level);
        let strip = |installed: &mut BTreeMap<u8, u32>, off: &dyn Fn(u8) -> bool| {
            for l in crate::frontend::graphics::VINYL_LAYERS {
                if part(installed, l).is_some_and(|p| off(p.kind)) {
                    installed.remove(&l);
                }
            }
        };
        match slot {
            0x1D => set(&mut self.installed, 0x1E, hash),
            0x22 => {
                for s in 0x23u8..=0x2E {
                    let h = crate::car::trunk_component(db, &car, s as usize).unwrap_or(0);
                    set(&mut self.installed, s, h);
                }
            }
            9 => {
                if db
                    .find(&car, 9, Some(hash), None)
                    .and_then(|p| db.attribute(p, crate::car::CARBONFIBRE))
                    .is_some_and(|v| v != 0)
                {
                    strip(&mut self.installed, &|k| k == 0x14);
                }
            }
            0x5D if level(&self.installed, 0x5D) > 0 => {
                if level(&self.installed, 6) > 0 {
                    strip(&mut self.installed, &|k| k != 0x14);
                } else {
                    strip(&mut self.installed, &|k| matches!(k, 0x0E | 0x0F | 0x15..=0x1A));
                }
            }
            6 if level(&self.installed, 0x5D) > 0 => strip(&mut self.installed, &|k| k != 0x14),
            _ => {}
        }
    }

    /// A visual shop action's end (the body shop's fits 0x55C240, the
    /// rims 0x567A60 and spinners 0x569680, the gauges 0x563D00, the setup
    /// purchase 0x547580 of paints, vinyls and decals …): the career ride
    /// (0x8392C0, every fit by 0x631D30 → 0x61BCE0) is copied into the
    /// car's record by 0x503950 with no 0x639CF0 refresh. So the record
    /// takes the derived slots (a wide body's doors, sills and decals, no
    /// wide body's first door parts, the hood's HOODUNDER), slot 0x0D /
    /// 0x1B from the performance parts, and the ride's upgrade state as
    /// built on entering the shop: category 9 from the body parts then
    /// (`shop_ride`, else as before this action) and the ten floats
    /// 0x5B4FD0 wrote for it. **Verified** byte for byte against the
    /// original's saves (`docs/notes/front-end/shop-record-store.md`). Outside a career
    // prov: open frontend.career.store_visual
    /// (Customize) only the part slots (the record there is **open**).
    pub fn store_visual(
        &mut self,
        db: &crate::formats::parts::PartDb,
        packages: &[crate::performance::Package],
        before: &BTreeMap<u8, u32>,
    ) {
        if self.car.is_empty() {
            return;
        }
        if !self.scratch && !packages.is_empty() {
            let built = self.shop_ride.clone().unwrap_or_else(|| before.clone());
            let car = self.car.clone();
            let level =
                |slot: u8| built.get(&slot).and_then(|&h| db.find(&car, slot as usize, Some(h), None)).map(|p| p.level);
            crate::performance::fit_aero(&mut self.perf, packages, &level);
            let perf = self.perf.clone();
            self.record_levels = Some(std::array::from_fn(|c| {
                crate::performance::state_float(packages, &|id| perf.contains(&id), c as u8)
            }));
        }
        self.store_parts(db, packages);
    }

    /// Drive the car of another slot.
    pub fn select_slot(&mut self, i: usize) {
        if i < self.slots.len() && self.slots[i].car.is_some() {
            self.stash();
            self.load_slot(i);
        }
    }

    /// Put a stock car in slot `i` (0x5348F0 / 0x534960): the slot's old
    /// car and its parts go; the slot becomes the current one.
    pub fn take_stock_car(&mut self, i: usize, car: &str) {
        self.stash();
        if let Some(s) = self.slots.get_mut(i) {
            // The spend (+0x7F0) stays: no code but 0x547747 writes it.
            *s = CarSlot { unlocked: true, car: Some(car.into()), spent: s.spent, ..CarSlot::default() };
            self.load_slot(i);
        }
    }

    /// The first open slot without a car (0x495FD0).
    pub fn free_slot(&self) -> Option<usize> {
        self.slots.iter().position(|s| s.unlocked && s.car.is_none() && !s.opaque)
    }

    /// The open slots with a car, the current one's state included.
    /// Every car slot as the crib's Car Select lists it (0x4E0160 over
    /// 0x5162D0 filter 4): (index, unlocked, car).
    pub fn all_slots(&self) -> Vec<(usize, bool, Option<String>)> {
        self.slots.iter().enumerate().map(|(i, s)| (i, s.unlocked, self.stashed_car(i))).collect()
    }

    /// Career slot `i`'s car with its parts and fitted performance parts
    /// (the current slot's from the profile itself): the ride the car
    /// lot's list entry builds from the slot's car record (0x511E00:
    /// 0x503510 then the record's vtable +0x0C) and copies into the FE car
    /// 0x8371B0 (0x413440 at 0x4DF8AC / 0x4DFBC0).
    pub fn slot_ride(&self, i: usize) -> Option<(String, BTreeMap<u8, u32>, BTreeSet<u8>)> {
        if i == self.slot {
            return (!self.car.is_empty()).then(|| (self.car.clone(), self.installed.clone(), self.perf.clone()));
        }
        let s = self.slots.get(i)?;
        Some((s.car.clone()?, s.installed.clone(), s.perf.clone()))
    }

    pub fn car_slots(&self) -> Vec<(usize, String)> {
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, s)| s.unlocked)
            .filter_map(|(i, _)| self.stashed_car(i).map(|c| (i, c)))
            .collect()
    }
}

#[cfg(test)]
mod stashed_view_tests {
    use super::*;

    /// The slot queries answer as the old whole-profile
    /// clone + stash did, for a current slot with and without a car.
    #[test]
    fn slot_views_answer_as_a_stashed_clone() {
        let mut p = Profile::new("TEST");
        p.slots = (0..CAR_SLOTS).map(|_| CarSlot::default()).collect();
        p.slots[0] = CarSlot { unlocked: true, car: Some("PEUGOT".into()), ..CarSlot::default() };
        p.slots[1] = CarSlot { unlocked: true, car: Some("OLD".into()), spent: 7, ..CarSlot::default() };
        p.slots[2].unlocked = true;
        p.slots[3] = CarSlot { unlocked: false, car: Some("LOCKED".into()), opaque: true, ..CarSlot::default() };
        for (slot, car) in [(1, "SKYLINE"), (1, ""), (2, "RX8"), (0, "")] {
            p.slot = slot;
            p.car = car.into();
            p.installed = [(3u8, 0x1234u32)].into_iter().collect();
            p.perf = [1u8, 4].into_iter().collect();
            p.brands = vec![9, 8];
            p.spent = 55;
            p.record_levels = Some([0.5; 10]);
            let mut q = p.clone();
            q.stash();
            let old_all: Vec<_> = q.slots.iter().enumerate().map(|(i, s)| (i, s.unlocked, s.car.clone())).collect();
            assert_eq!(p.all_slots(), old_all, "{slot} {car}");
            let old_cars: Vec<_> = q
                .slots
                .iter()
                .enumerate()
                .filter(|(_, s)| s.unlocked)
                .filter_map(|(i, s)| s.car.clone().map(|c| (i, c)))
                .collect();
            assert_eq!(p.car_slots(), old_cars, "{slot} {car}");
            for i in 0..CAR_SLOTS {
                assert_eq!(format!("{:?}", &*p.stashed_slot(i).unwrap()), format!("{:?}", q.slots[i]), "{slot} {car} {i}");
            }
            assert!(p.stashed_slot(CAR_SLOTS).is_none());
        }
    }
}
