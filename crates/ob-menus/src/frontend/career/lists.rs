//! Car lists: the car lot and Quick Race.

use super::*;

/// Car type indices the career car lot never offers (0x5022C0): the
/// table at 0x7F7C50, CIVIC and RSX, when 0x570A40 (the region) is not 0.
pub const LOT_EXCLUDED: [usize; 2] = [5, 0x12];
/// The same for region 0 (0x7F7C48): CORSA and PEUGOT106.
pub const LOT_EXCLUDED_US: [usize; 2] = [7, 6];
/// The car type ids Quick Race offers without a career unlock (0x529CD0's
/// list 0x7F7C28, taken when the region is not 0).
pub const QR_ALWAYS: [usize; 8] = [0, 1, 2, 7, 10, 25, 28, 22];
/// The same for region 0 (0x7F7C08): CIVIC (5) in place of CORSA (7).
pub const QR_ALWAYS_US: [usize; 8] = [0, 1, 2, 5, 10, 25, 28, 22];

/// The lot's excluded car types in `region` ([`crate::region`]).
pub fn lot_excluded(region: u32) -> &'static [usize; 2] {
    if region == 0 { &LOT_EXCLUDED_US } else { &LOT_EXCLUDED }
}

/// Quick Race's always-offered car types in `region`.
pub fn qr_always(region: u32) -> &'static [usize; 8] {
    if region == 0 { &QR_ALWAYS_US } else { &QR_ALWAYS }
}

impl Profile {
    /// The cars the career car lot offers (0x4DF9C0 in list mode 1): the
    /// stock cars (0x516500) in car type order — the sort 0x43E650 with
    /// the always-true 0x40CB20 keeps it — less the two 0x5022C0 drops
    /// and those whose unlock event (0x513C50: the 0x34A1F record's +8)
    /// is an event not yet won.
    pub fn lot_cars(&self, data: &CareerData, types: &[crate::car::CarType]) -> Vec<usize> {
        self.lot_cars_in(crate::region::get(), data, types)
    }

    /// [`Profile::lot_cars`] in `region` (its exclusions, [`lot_excluded`]).
    pub fn lot_cars_in(&self, region: u32, data: &CareerData, types: &[crate::car::CarType]) -> Vec<usize> {
        let excluded = lot_excluded(region);
        (0..types.len())
            .filter(|&i| types[i].stock_car && !excluded.contains(&i))
            .filter(|&i| {
                let hash = bin_hash(&types[i].name);
                data.unlocks.iter().filter(|(car, _)| *car == hash).all(|&(_, trigger)| {
                    trigger == 0 || !data.events.iter().any(|e| e.unlock == trigger) || self.won(data, trigger)
                })
            })
            .collect()
    }

    /// The stock cars UI_QRCarSelect lists (0x4E22E0 → 0x534190 with the
    /// Quick Race mask 1): those 0x529F80 passes (the career unlocks,
    /// [`Profile::lot_cars`]) or 0x529CD0 does — the car type ids in the
    /// list 0x7F7C28 (0, 1, 2, 7, 10, 25, 28, 22: 206, Focus, Corolla,
    /// Corsa, Escalade, GTO, Lancer Evolution VIII, 350Z; 0x7F7C08, with 5
    /// for 7, in region 0: [`qr_always`]) — in car type order. Wine (menus15, a fresh
    /// profile): 206, Focus, Corolla, 240SX, 106, Corsa, Escalade, 350Z,
    /// GTO, Evolution VIII; the clone listed the lot's six. With regmebaby
    /// typed at the title 0x529F80 also passes a stock record of type 0x15
    /// or 0x1B (0x579D40: RX8, SKYLINE; under Wine they came in after
    /// Escalade and GTO).
    pub fn quick_race_cars(&self, data: &CareerData, types: &[crate::car::CarType]) -> Vec<usize> {
        self.quick_race_cars_in(crate::region::get(), data, types)
    }

    /// [`Profile::quick_race_cars`] in `region` ([`lot_excluded`],
    /// [`qr_always`]).
    pub fn quick_race_cars_in(&self, region: u32, data: &CareerData, types: &[crate::car::CarType]) -> Vec<usize> {
        let lot = self.lot_cars_in(region, data, types);
        let (excluded, always) = (lot_excluded(region), qr_always(region));
        (0..types.len())
            .filter(|&i| types[i].stock_car && !excluded.contains(&i))
            .filter(|i| lot.contains(i) || always.contains(i) || crate::frontend::cheats::car_type(*i))
            .collect()
    }
}

#[cfg(test)]
mod region_tests {
    use super::*;
    use crate::region::{EUROPE, US};

    /// Region 0's tables (0x7F7C48, 0x7F7C08, 0x7F6DA4) and the European
    /// ones (0x7F7C50, 0x7F7C28, 0x7F6DC0): the pools differ only in their
    /// last two cars, CIVIC and RSX against CORSA and PEUGOT106.
    #[test]
    fn region_tables() {
        assert_eq!(lot_excluded(EUROPE), &[5, 0x12]);
        assert_eq!(lot_excluded(US), &[7, 6]);
        assert_eq!(qr_always(EUROPE), &[0, 1, 2, 7, 10, 25, 28, 22]);
        assert_eq!(qr_always(US), &[0, 1, 2, 5, 10, 25, 28, 22]);
        let (us, eu) = (crate::opponents::car_pool_in(US, false), crate::opponents::car_pool_in(EUROPE, false));
        assert_eq!(&us[24..], &[5, 0x12]);
        assert_eq!(&eu[24..], &[7, 6]);
        assert_eq!(us[..24], eu[..24]);
        assert_eq!(crate::opponents::car_pool_in(US, true), crate::opponents::car_pool_in(EUROPE, true));
    }

    /// The European install is region 6, and the default lists are the
    /// European ones; in region 0 the lot and Quick Race offer CIVIC and RSX
    /// where the European ones offer CORSA and PEUGOT106.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn lists_by_region() {
        let root = crate::testkit::game_root();
        let data = CareerData::load(&root).unwrap();
        assert_eq!(crate::region::get(), EUROPE);
        let globalb = crate::car::shared_file(&crate::game_fs::path(&root, "GLOBAL/GLOBALB.BUN")).unwrap();
        let types = crate::car::parse_car_types(&globalb).unwrap();
        let p = Profile::new("T");
        let id = |name: &str| types.iter().position(|t| t.name == name).unwrap();
        assert_eq!((id("CIVIC"), id("RSX"), id("CORSA"), id("PEUGOT106")), (5, 0x12, 7, 6));
        let (eu_lot, us_lot) = (p.lot_cars_in(EUROPE, &data, &types), p.lot_cars_in(US, &data, &types));
        assert_eq!(p.lot_cars(&data, &types), eu_lot);
        assert_eq!(p.quick_race_cars(&data, &types), p.quick_race_cars_in(EUROPE, &data, &types));
        let eu_qr = p.quick_race_cars_in(EUROPE, &data, &types);
        let us_qr = p.quick_race_cars_in(US, &data, &types);
        for c in [5, 0x12] {
            assert!(!eu_lot.contains(&c) && !eu_qr.contains(&c));
        }
        for c in [7, 6] {
            assert!(!us_lot.contains(&c) && !us_qr.contains(&c));
        }
        assert!(eu_qr.contains(&7) && us_qr.contains(&5));
        let strip = |v: &[usize]| v.iter().copied().filter(|c| ![5, 6, 7, 0x12].contains(c)).collect::<Vec<_>>();
        assert_eq!(strip(&eu_lot), strip(&us_lot));
        assert_eq!(strip(&eu_qr), strip(&us_qr));
    }
}
