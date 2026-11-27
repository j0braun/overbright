//! The speech library's request evaluation (the EA library at
//! 0x734E70..0x738A40 that both speech banks go through), from a switch's
//! tuple to the record that plays. **Verified** in the disassembly
//! (traced for the native fixes; evidence: docs/native-fixes/N-AUD2.tsv):
//!
//! 1. A game setter stores a switch's tuple (0x735880) and the switch's
//!    listener 0x738630 (installed per switch by 0x7386A0: switch i of the
//!    `.evt` → event i) builds the request {event key, the event's +0xB
//!    tuple values} and posts it (0x736920).
//! 2. 0x736920: the request passes when Rand(100) ≤ event +9 (0x73698A),
//!    then waits in the channel's queue (0x736860: lifetime +2, priority
//!    +4; not modelled — the clone evaluates at once).
//! 3. 0x738340 (from 0x7385B0): 0x737470 orders the alternatives by
//!    weight (one Rand(total) per non-zero weight, the drawn one removed;
//!    zero weights last in file order); each in that order is tried:
//!    Rand(100) < its chance (0x738489), the switch masks (0x7382A0), the
//!    key fit (0x738DD0), then 0x7377B0 → 0x737230 builds the step's
//!    candidates and 0x737B20 picks one; the first alternative that plays
//!    ends the request. A failed request plays nothing (the retry hook
//!    0x8B5158 is never set).
//! 4. 0x737230: the records that fit the step (`Bank::fitting`); one
//!    played within the entry's last +6 plays (0x735D70) is only a
//!    fallback: with no other, the one played longest ago is used.
//! 5. 0x737B20: record = candidates[pick(count, entry)] (0x738860 with
//!    its 32-entry history); 0x737EEB notes it in the entry's ring
//!    (0x735D30) when it starts.
//!
//! Every Rand is 0x738860 on the library's generator 0x738750
//! ([`super::Picker`]), shared by the speech and NIS banks; its seed comes
//! from the game's generator 0x7EA624 at each library init 0x486220.

use super::{Bank, Event, Picker};
use std::collections::HashMap;

/// The draws a request makes, in their order (the real one is
/// [`Picker`]; tests record them).
pub trait Draws {
    /// 0x738860(n, −1).
    fn rand(&mut self, n: u32) -> u16;
    /// 0x738860(n, key).
    fn pick(&mut self, n: u32, key: u16) -> u16;
}

impl Draws for Picker {
    fn rand(&mut self, n: u32) -> u16 {
        Picker::rand(self, n)
    }
    fn pick(&mut self, n: u32, key: u16) -> u16 {
        Picker::pick(self, n, key)
    }
}

/// The two banks' slots: their entries' history keys must not collide
/// (natively the entry's index in the library's table 0x8B5174).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BankId {
    Nis = 0,
    Speech = 1,
}

/// A request's result: the entry key and record that play.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pick {
    pub event: u16,
    pub entry: u32,
    pub record: usize,
}

/// 0x737470: the alternatives' order.
pub fn weighted_order(ev: &Event, d: &mut dyn Draws) -> Vec<usize> {
    let mut w: Vec<u32> = ev.alternatives.iter().map(|a| a.weight()).collect();
    let mut total: u32 = w.iter().sum();
    let mut order = Vec::new();
    while total > 0 {
        let mut r = d.rand(total) as i64;
        let mut i = 0;
        while i < w.len() {
            r -= w[i] as i64;
            if r < 0 {
                break;
            }
            i += 1;
        }
        // The draw is below the total, so some weight takes it.
        let i = i.min(w.len() - 1);
        order.push(i);
        total -= w[i];
        w[i] = 0;
    }
    order.extend(ev.alternatives.iter().enumerate().filter(|(_, a)| a.weight() == 0).map(|(i, _)| i));
    order
}

/// The state the library keeps between requests: its generator, the
/// init counter 0x82BA34 and the entries' recently played rings.
#[derive(Clone, Debug)]
pub struct Library {
    pub picker: Picker,
    counter: u32,
    rings: HashMap<(BankId, usize), (u8, Vec<u8>)>,
}

/// The seed until the game runs the library's init (check aid for
/// headless runs; natively the library is not started before 0x486220).
pub const UNINIT_SEED: u32 = 10;

impl Default for Library {
    fn default() -> Library {
        Library { picker: Picker::new(UNINIT_SEED), counter: 0, rings: HashMap::new() }
    }
}

impl Library {
    /// 0x486220 (from 0x4866EA and 0x486A6D, each right after the game's
    /// Rand_Int(0xFFFFFFFF) at 0x4866F0 / 0x486A2C on 0x7EA624 → `draw`):
    /// the first init seeds with the draw (0x43C140 on that value), every
    /// later one with the last seed + 1 (0x82BA34, .bss 0); the library
    /// start 0x736180 → 0x738940(|seed|) restarts the generator and
    /// clears its history.
    pub fn init(&mut self, draw: u32) -> u32 {
        let seed = if self.counter == 0 { draw } else { self.counter.wrapping_add(1) };
        self.counter = seed;
        let seed = (seed as i32).unsigned_abs();
        self.picker = Picker::new(seed);
        seed
    }

    fn ring<'a>(&'a mut self, bank: &Bank, id: BankId, entry: usize) -> &'a mut (u8, Vec<u8>) {
        self.rings.entry((id, entry)).or_insert_with(|| {
            let e = &bank.entries[entry];
            let mut r = e.ring.clone();
            r.1.resize(e.history as usize, 0xFF);
            r
        })
    }

    /// 0x735D70: how many plays ago record `r` was played (1 the last),
    /// 0 when it is not in the ring.
    pub fn age(&mut self, bank: &Bank, id: BankId, entry: usize, r: usize) -> u32 {
        let depth = bank.entries[entry].history as i32;
        let (pos, ring) = self.ring(bank, id, entry);
        let mut at = *pos as i32;
        for d in 0..depth {
            at -= 1;
            if at < 0 {
                at = depth - 1;
            }
            if ring.get(at as usize).is_some_and(|&x| x as usize == r) {
                return d as u32 + 1;
            }
        }
        0
    }

    /// 0x735D30 (from the play step 0x737EEB): the record joins the
    /// entry's ring.
    pub fn played(&mut self, bank: &Bank, id: BankId, pick: Pick) {
        let Some(entry) = bank.entry_index(pick.entry) else { return };
        let depth = bank.entries[entry].history;
        if depth == 0 {
            return;
        }
        let (pos, ring) = self.ring(bank, id, entry);
        if *pos < depth {
            ring[*pos as usize] = pick.record as u8;
        }
        *pos = if *pos + 1 >= depth { 0 } else { *pos + 1 };
    }

    /// Steps 2–5 of the module doc for event `ev` with the request
    /// `values`, drawing from the library's generator.
    pub fn request(&mut self, bank: &Bank, id: BankId, ev: &Event, values: &[u32]) -> Option<Pick> {
        let mut picker = self.picker.clone();
        let r = self.request_with(bank, id, ev, values, &mut picker);
        self.picker = picker;
        r
    }

    /// [`Self::request`] with the draws taken from `d`.
    pub fn request_with(
        &mut self,
        bank: &Bank,
        id: BankId,
        ev: &Event,
        values: &[u32],
        d: &mut dyn Draws,
    ) -> Option<Pick> {
        // 0x73698A: Rand(100) ≤ the event's (signed) +9.
        if d.rand(100) as i32 > ev.chance as i32 {
            return None;
        }
        if !ev.ported() {
            return None;
        }
        // 0x738340 (the count limit and the channel state are not reached
        // by the ported events: see `Event::ported`).
        let order = weighted_order(ev, d);
        for &a in order.iter().take(ev.alternatives.len()) {
            let alt = &ev.alternatives[a];
            if d.rand(100) >= alt.chance as u16 {
                continue;
            }
            // 0x7382A0 / 0x738DD0 pass with no switch masks and no keys.
            let step = &alt.steps[0];
            let Some(key) = bank.resolve_entry(step, values) else { continue };
            let Some(entry) = bank.entry_index(key) else { continue };
            // 0x737230: candidates, else the fallback played longest ago.
            let mut cands = Vec::new();
            let mut fallback = (0, None);
            for r in bank.fitting(step, key, values) {
                match self.age(bank, id, entry, r) {
                    0 => {
                        if cands.len() < 200 {
                            cands.push(r);
                        }
                    }
                    age if age > fallback.0 => fallback = (age, Some(r)),
                    _ => {}
                }
            }
            if cands.is_empty()
                && let Some(r) = fallback.1
            {
                cands.push(r);
            }
            if cands.is_empty() {
                continue;
            }
            // 0x737B20: candidates[pick(count, the entry's slot)].
            let identity = ((id as usize) << 8 | entry) as u16;
            let i = d.pick(cands.len() as u32, identity) as usize;
            return Some(Pick { event: ev.id, entry: key, record: cands[i.min(cands.len() - 1)] });
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use super::*;

    /// Records the draws: `Rand(n)` returns `rand[i]`, `pick` returns 0.
    struct Log {
        calls: Vec<(u32, Option<u16>)>,
        rand: Vec<u16>,
    }

    impl Draws for Log {
        fn rand(&mut self, n: u32) -> u16 {
            self.calls.push((n, None));
            let i = self.calls.iter().filter(|c| c.1.is_none()).count() - 1;
            self.rand.get(i).copied().unwrap_or(0)
        }
        fn pick(&mut self, n: u32, key: u16) -> u16 {
            self.calls.push((n, Some(key)));
            0
        }
    }

    fn step(entry: u16, constraints: &[(u32, u8)]) -> Step {
        Step {
            entry,
            entry_component: 0,
            entry_mode: 0,
            constraints: constraints
                .iter()
                .map(|&(mask, component)| Constraint { mask, component, slot: 0xFF })
                .collect(),
        }
    }

    fn alt(weight: u8, chance: u8, s: Step) -> Alternative {
        Alternative { weight, chance, mode: 0, state: 0, steps: vec![s] }
    }

    fn event(alts: Vec<Alternative>) -> Event {
        Event {
            id: 0x11,
            lifetime: 200,
            priority: 500,
            keys: 0,
            limits: 0,
            chance: 100,
            flags: 0,
            values: 2,
            alternatives: alts,
        }
    }

    fn entry(id: u16, keys: &[[u8; 2]], history: u8) -> Entry {
        Entry {
            id,
            selector: 0xFFFF,
            keys: 2,
            records: keys.iter().enumerate().map(|(i, k)| (i as u32 * 0x100, k.to_vec())).collect(),
            end: keys.len() as u32 * 0x100,
            base: 0,
            history,
            ring: (0, vec![0xFF; history as usize]),
            unions: vec![u32::MAX, u32::MAX],
            enabled: None,
        }
    }

    #[test]
    fn a_request_draws_gate_order_chance_then_pick() {
        // 0x73698A Rand(100); 0x7374E3 Rand(total) once per non-zero
        // weight (0x39 → 25 · 4 = 100); 0x738489 Rand(100) per tried
        // alternative; 0x737B20 pick(count, entry).
        let bank = Bank { entries: vec![entry(0x27, &[[0, 1], [0, 1], [0, 2]], 0)], ..Bank::default() };
        let ev = event(vec![alt(0x39, 100, step(0x27, &[(1, 1), (0xFF, 2)]))]);
        let mut lib = Library::default();
        let mut log = Log { calls: Vec::new(), rand: vec![] };
        let p = lib.request_with(&bank, BankId::Speech, &ev, &[1, 2], &mut log).unwrap();
        assert_eq!(log.calls, [(100, None), (100, None), (100, None), (2, Some(0x100))]);
        assert_eq!(p.record, 0);
        // The gate: Rand(100) ≤ +9 passes, above it nothing is drawn more.
        let mut gated = ev.clone();
        gated.chance = 40;
        let mut log = Log { calls: Vec::new(), rand: vec![41] };
        assert!(lib.request_with(&bank, BankId::Speech, &gated, &[1, 2], &mut log).is_none());
        assert_eq!(log.calls.len(), 1);
    }

    #[test]
    fn alternatives_go_by_weight_and_their_chance() {
        // 0x737470: weights 1 · 4^0 = 1 and (3 · 4^1) = 12 → total 13; a
        // draw of 5 takes the second (5 − 1 = 4 ≥ 0, 4 − 12 < 0), then the
        // first (total 1), then the zero weight in file order.
        let ev = event(vec![alt(0x01, 100, step(1, &[])), alt(0x23, 100, step(2, &[])), alt(0, 100, step(3, &[]))]);
        let mut log = Log { calls: Vec::new(), rand: vec![5, 0] };
        assert_eq!(weighted_order(&ev, &mut log), [1, 0, 2]);
        assert_eq!(log.calls, [(13, None), (1, None)]);
        // 0x738489: an alternative is tried when Rand(100) < its chance;
        // a failed one passes to the next of the order.
        let bank = Bank { entries: vec![entry(1, &[[0, 0]], 0), entry(2, &[[0, 0]], 0)], ..Bank::default() };
        let ev = event(vec![alt(0x01, 30, step(1, &[])), alt(0x01, 100, step(2, &[]))]);
        let mut lib = Library::default();
        // gate 0, order draw 0 (→ alternative 0), then 1 → [0, 1]; its
        // chance draw 30 fails (30 < 30 is false); the next draw 99 < 100.
        let mut log = Log { calls: Vec::new(), rand: vec![0, 0, 0, 30, 99] };
        let p = lib.request_with(&bank, BankId::Speech, &ev, &[], &mut log).unwrap();
        assert_eq!(p.entry, 2);
    }

    #[test]
    fn recently_played_records_are_only_a_fallback() {
        // 0x737230 / 0x735D70: entry +6 = 2. After records 0 and 1 played,
        // record 2 is the only candidate; with all three recent, the one
        // played longest ago (age 2) is used.
        let bank = Bank { entries: vec![entry(0x2D, &[[0, 0], [0, 0], [0, 0]], 2)], ..Bank::default() };
        let ev = event(vec![alt(0x39, 100, step(0x2D, &[(1, 0), (1, 0)]))]);
        let mut lib = Library::default();
        for r in [0, 1] {
            lib.played(&bank, BankId::Speech, Pick { event: 0x11, entry: 0x2D, record: r });
        }
        assert_eq!((lib.age(&bank, BankId::Speech, 0, 1), lib.age(&bank, BankId::Speech, 0, 0)), (1, 2));
        let mut log = Log { calls: Vec::new(), rand: vec![] };
        let p = lib.request_with(&bank, BankId::Speech, &ev, &[], &mut log).unwrap();
        assert_eq!((p.record, log.calls.last().copied()), (2, Some((1, Some(0x100)))));
        // Ring of 2: playing 2 pushes 0 out; 1 (age 2) is the fallback when
        // only records 1 and 2 fit.
        lib.played(&bank, BankId::Speech, p);
        assert_eq!(lib.age(&bank, BankId::Speech, 0, 0), 0);
        let one_two = Bank { entries: vec![entry(0x2D, &[[1, 0], [0, 0], [0, 0]], 2)], ..Bank::default() };
        let ev = event(vec![alt(0x39, 100, step(0x2D, &[(1, 0)]))]);
        let mut lib2 = lib.clone();
        let mut log = Log { calls: Vec::new(), rand: vec![] };
        let p = lib2.request_with(&one_two, BankId::Speech, &ev, &[], &mut log).unwrap();
        assert_eq!(p.record, 1);
    }

    #[test]
    fn the_library_init_seeds_from_the_game_draw_then_counts_up() {
        // 0x486220: first the game's draw, then the last seed + 1; |seed|
        // as a signed value (0x48627A).
        let mut lib = Library::default();
        assert_eq!(lib.init(0xFFFF_FFF0), 16);
        assert_eq!(lib.init(1234), 15);
        let mut a = Library::default();
        a.init(77);
        let mut p = Picker::new(77);
        assert_eq!(a.picker.draw(), p.draw());
    }
}
