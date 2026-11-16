//! Car lists: the car lot and Quick Race.

use super::*;

/// Car type indices the career car lot never offers (0x5022C0: the table
/// at 0x7F7C50, CIVIC and RSX, taken because 0x570A40 is non-zero; the
/// other build's table 0x7F7C48 holds PEUGOT106 and CORSA).
pub const LOT_EXCLUDED: [usize; 2] = [5, 0x12];
/// The car type ids Quick Race offers without a career unlock (0x529CD0's
/// list 0x7F7C28, the region-nonzero one; the file's region 0x7FA73C is 6).
pub const QR_ALWAYS: [usize; 8] = [0, 1, 2, 7, 10, 25, 28, 22];

impl Profile {
    /// The cars the career car lot offers (0x4DF9C0 in list mode 1): the
    /// stock cars (0x516500) in car type order — the sort 0x43E650 with
    /// the always-true 0x40CB20 keeps it — less the two 0x5022C0 drops
    /// and those whose unlock event (0x513C50: the 0x34A1F record's +8)
    /// is an event not yet won.
    pub fn lot_cars(&self, data: &CareerData, types: &[crate::car::CarType]) -> Vec<usize> {
        (0..types.len())
            .filter(|&i| types[i].stock_car && !LOT_EXCLUDED.contains(&i))
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
    /// for 7, is region 0's) — in car type order. Wine (menus15, a fresh
    /// profile): 206, Focus, Corolla, 240SX, 106, Corsa, Escalade, 350Z,
    /// GTO, Evolution VIII; the clone listed the lot's six. With regmebaby
    /// typed at the title 0x529F80 also passes a stock record of type 0x15
    /// or 0x1B (0x579D40: RX8, SKYLINE; under Wine they came in after
    /// Escalade and GTO).
    pub fn quick_race_cars(&self, data: &CareerData, types: &[crate::car::CarType]) -> Vec<usize> {
        let lot = self.lot_cars(data, types);
        (0..types.len())
            .filter(|&i| types[i].stock_car && !LOT_EXCLUDED.contains(&i))
            .filter(|i| lot.contains(i) || QR_ALWAYS.contains(i) || crate::frontend::cheats::car_type(*i))
            .collect()
    }
}
