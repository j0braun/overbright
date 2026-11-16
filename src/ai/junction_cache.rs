//! Controller+14's sixteen ordered words (41CFA0), shared by route goals.
//! Eligibility remains at the caller. See docs/outrun-cache-capacity.md.
use std::sync::atomic::{AtomicI32, Ordering};

// 827AB8 is image-initialized, not a per-controller counter. The original
// increments on every overflow, including past ten, with signed wrapping.
static OVERFLOWS: AtomicI32 = AtomicI32::new(0);

#[derive(Clone, Debug, Default)]
pub(super) struct JunctionCache {
    slots: [u32; 16],
}

impl JunctionCache {
    pub fn get(&self, junction: usize) -> Option<bool> {
        self.slots.iter().find(|&&w| w & 1 != 0 && ((w >> 4) & 0x1fff) as usize == junction)
            .map(|w| w & 8 != 0)
    }

    /// A cacheable miss only: callers have already evaluated the predicate.
    pub fn insert(&mut self, junction: usize, accepted: bool) {
        self.insert_counted(junction, accepted, &OVERFLOWS);
    }

    fn insert_counted(&mut self, junction: usize, accepted: bool, counter: &AtomicI32) {
        let slot = self.slots.iter().position(|w| w & 1 == 0).unwrap_or_else(|| {
            counter.fetch_add(1, Ordering::Relaxed);
            // Original old-count<10 calls41A090/40F6F0. Their geometry
            // results are discarded; lazy performance setup is already
            // performed by car_type/set_race before a clone route runs.
            // They do not reset route/cache storage (native helper oracle).
            0
        });
        self.slots[slot] = (self.slots[slot] & 0xfffe_0003)
            | (((junction as u32) & 0x1fff) << 4) | (u32::from(accepted) << 3) | 3;
    }

    /// Successful 41E540 import only; caller also replaces origin/list state.
    /// Traversal 41B552/41B598 bounds decisions to fifteen. Not wired yet.
    #[allow(dead_code)]
    pub fn import_search(&mut self, decisions: &[(usize, bool)]) {
        assert!(decisions.len() <= 15, "native search decision bound");
        self.clear();
        // Every slot is invalid after clear. Native's first-invalid walk
        // therefore consumes consecutive slots, including repeated IDs.
        for (word, &(junction, accepted)) in self.slots.iter_mut().zip(decisions) {
            *word = (*word & 0xfffe_0007) | (((junction as u32) & 0x1fff) << 4)
                | (u32::from(accepted) << 3) | 7;
        }
    }

    /// 41D0E0: expire only ordinary entries, retaining invalid slot identity.
    /// Bit2 belongs to search imports; importing them remains unwired.
    pub fn retain_ordinary(&mut self, mut keep: impl FnMut(usize) -> bool) {
        for word in &mut self.slots {
            if *word & 1 != 0 && *word & 4 == 0 && !keep(((*word >> 4) & 0x1fff) as usize) {
                *word &= !1;
            }
        }
    }

    /// Goal-5 exit callback 40DB50 -> 404970 clears all low flags only
    /// on words marked as search imports, including already-invalid ones.
    pub fn clear_search(&mut self) {
        for word in &mut self.slots {
            if *word & 4 != 0 { *word &= !7; }
        }
    }

    /// Route reset at42CEB0 clears bits0–1, not every stored bit.
    pub fn clear(&mut self) {
        for word in &mut self.slots { *word &= !3; }
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(|w| w & 1 == 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
    fn all_slots_and_counter_match_39_original_byte_calls() {
        let raw = &crate::testkit::private("docs/evidence/outrun/cache-helper-followup/native-cache.bin");
        let mut cache = JunctionCache::default();
        let counter = AtomicI32::new(0); // Never reads or resets the process counter.
        let mut calls = 0;
        let mut cases: Vec<(usize, Option<i32>, Option<usize>)> = (0..29).map(|j| (j,None,None)).collect();
        cases.extend([(1,None,None),(29,None,Some(7)),(30,Some(9),None),(31,Some(10),None),
            (32,Some(-1),None),(33,Some(i32::MAX),None),(34,Some(i32::MIN),None),
            (35,Some(0),None),(36,Some(10),Some(7)),(37,Some(10),None)]);
        assert_eq!(raw.len(), cases.len()*72);
        for (i, (junction, count, invalid)) in cases.into_iter().enumerate() {
            if i==36 { cache.slots[0] |= 0xabce_0004; }
            if i==37 { cache.slots[7] |= 0x5678_0004; }
            if let Some(count) = count { counter.store(count, Ordering::Relaxed); }
            if let Some(slot) = invalid { cache.slots[slot] &= !1; }
            if cache.get(junction).is_none() {
                calls += 1;
                cache.insert_counted(junction, i==38, &counter);
            }
            let words: Vec<u32> = raw[i*72..(i+1)*72].chunks_exact(4)
                .map(|w| u32::from_le_bytes(w.try_into().unwrap())).collect();
            assert_eq!(&cache.slots[..], &words[..16], "native case{i}");
            assert_eq!(counter.load(Ordering::Relaxed) as u32, words[16], "counter case{i}");
            assert_eq!(calls, words[17], "predicate draws case{i}");
        }
        assert_eq!(cache.get(37), Some(true));
    }

    #[test]
    #[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
    fn search_import_matches_all_original_slot_words() {
        let raw = &crate::testkit::private("docs/evidence/outrun/search-import-followup/import.bin");
        let mut case = 0;
        for count in [0, 1, 15] {
            for _direction in [0, 1, 2, 255] {
                for pattern in 0..3 {
                    let mut cache = JunctionCache { slots: std::array::from_fn(|i|
                        0xabc0_0000 + i as u32 * 0x20007 + pattern * 0x1019) };
                    let decisions: Vec<_> = (0..count).map(|i|
                        (if pattern == 0 { i } else if pattern == 1 { 8191 } else { 7 }, i % 2 != 0)).collect();
                    cache.import_search(&decisions);
                    let words: Vec<u32> = raw[case*76..(case+1)*76].chunks_exact(4)
                        .map(|w| u32::from_le_bytes(w.try_into().unwrap())).collect();
                    assert_eq!(&cache.slots[..], &words[3..], "native import case {case}");
                    case += 1;
                }
            }
        }
        assert_eq!(case*76, raw.len());
    }

    #[test]
    fn expiry_preserves_slot_identity_search_flags_and_high_bits() {
        let mut cache = JunctionCache::default();
        let counter = AtomicI32::new(0);
        for j in 0..16 { cache.insert_counted(j, j % 2 == 0, &counter); }
        cache.slots[3] |= 0x8000_0004; // Imported search entry is exempt.
        cache.slots[7] |= 0x4000_0000;
        let before = cache.slots;
        cache.retain_ordinary(|j| j!=3 && j!=7);
        assert_eq!(cache.slots[3], before[3]);
        assert_eq!(cache.slots[7], before[7] & !1);
        cache.insert_counted(20, true, &counter);
        assert_eq!(cache.slots[7], 0x4000_014b);
        assert_eq!(&cache.slots[..7], &before[..7]);
        assert_eq!(&cache.slots[8..], &before[8..]);
        assert_eq!(counter.load(Ordering::Relaxed), 0);
        let before = cache.slots;
        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.slots, before.map(|word| word & !3));
        assert_eq!(counter.load(Ordering::Relaxed), 0, "route reset cannot reset global overflow count");
    }

    #[test]
    fn counter_is_shared_between_controllers_and_wraps_without_saturation() {
        let counter = AtomicI32::new(i32::MAX);
        let mut first = JunctionCache { slots: [3;16] };
        let mut second = first.clone();
        first.insert_counted(1, false, &counter);
        assert_eq!(counter.load(Ordering::Relaxed), i32::MIN);
        second.insert_counted(2, false, &counter);
        assert_eq!(counter.load(Ordering::Relaxed), i32::MIN+1);
        first.clear();
        first.insert_counted(3, false, &counter);
        assert_eq!(counter.load(Ordering::Relaxed), i32::MIN+1);
    }
}
