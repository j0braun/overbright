//! The world renderer's texture slots past its own textures
//! ([`WorldRenderer::add_texture_slot`](super::WorldRenderer::add_texture_slot)
//! and the cars' shadows): a table whose freed entries are taken again, so
//! the scenes' characters, whose textures go with their scene (the NIS
//! unload 0x43B640 → 0x437660 unloads the scene's bundles through
//! 0x580EA0), leave nothing behind over a session.

use std::sync::{Arc, Mutex};

/// The slots past the world's own `base` textures: slot `base + i` is
/// entry `i`.
pub(in crate::render) struct ExtraSlots<T> {
    base: usize,
    items: Vec<Option<T>>,
    free: Vec<usize>,
    returned: Arc<Mutex<Vec<usize>>>,
}

/// Where a holder of slots gives them back (when it is dropped, without
/// the renderer at hand); the renderer takes them again with its next slot.
#[derive(Clone)]
pub struct SlotReturns {
    base: usize,
    returned: Arc<Mutex<Vec<usize>>>,
}

impl SlotReturns {
    /// Whether these are `slots`' returns (the table they were taken
    /// from: the very renderer, kept alive by this handle's share).
    pub(in crate::render) fn is_of<T>(&self, slots: &ExtraSlots<T>) -> bool {
        Arc::ptr_eq(&self.returned, &slots.returned)
    }

    /// The slots (as [`ExtraSlots::insert`] gave them) free again.
    pub fn give_back(&self, slots: impl IntoIterator<Item = usize>) {
        let mut r = self.returned.lock().unwrap_or_else(|e| e.into_inner());
        r.extend(slots.into_iter().filter_map(|s| s.checked_sub(self.base)));
    }
}

impl<T> ExtraSlots<T> {
    pub(in crate::render) fn new(base: usize) -> ExtraSlots<T> {
        ExtraSlots { base, items: Vec::new(), free: Vec::new(), returned: Default::default() }
    }

    pub(in crate::render) fn returns(&self) -> SlotReturns {
        SlotReturns { base: self.base, returned: self.returned.clone() }
    }

    /// The given-back entries emptied (their GPU objects dropped).
    fn reclaim(&mut self) {
        let back = std::mem::take(&mut *self.returned.lock().unwrap_or_else(|e| e.into_inner()));
        for i in back {
            if let Some(item) = self.items.get_mut(i)
                && item.take().is_some()
            {
                self.free.push(i);
            }
        }
    }

    /// `item` in a free entry (else a new one at the end): its slot.
    pub(in crate::render) fn insert(&mut self, item: T) -> usize {
        self.reclaim();
        let i = match self.free.pop() {
            Some(i) => {
                self.items[i] = Some(item);
                i
            }
            None => {
                self.items.push(Some(item));
                self.items.len() - 1
            }
        };
        self.base + i
    }

    /// Entry `i` (slot `base + i`); `None` once given back.
    pub(in crate::render) fn get(&self, i: usize) -> Option<&T> {
        self.items.get(i).and_then(Option::as_ref)
    }

    /// The slots held now (given-back ones not counted).
    pub(in crate::render) fn in_use(&mut self) -> usize {
        self.reclaim();
        self.items.iter().filter(|i| i.is_some()).count()
    }

    /// The table's length (held and free entries).
    #[cfg(test)]
    pub(in crate::render) fn table_len(&self) -> usize {
        self.items.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Scene after scene taking and giving back their slots, with a
    /// shadow's slot taken between them: the table stays at its first
    /// size and the slots held return to the count before the scenes.
    #[test]
    fn slots_given_back_are_taken_again() {
        let mut s = ExtraSlots::new(100);
        let shadow = s.insert("shadow");
        assert_eq!(shadow, 100);
        let start = s.in_use();
        for round in 0..20 {
            let r = s.returns();
            let slots: Vec<usize> = (0..7).map(|_| s.insert("character")).collect();
            assert!(slots.iter().all(|&k| (101..108).contains(&k)), "round {round}: {slots:?}");
            if round == 3 {
                s.insert("another shadow");
            }
            r.give_back(slots);
            assert_eq!(s.get(0), Some(&"shadow"));
        }
        assert_eq!(s.in_use(), start + 1);
        assert_eq!(s.table_len(), 9);
        // A slot below the base (the world's own) is not this table's.
        s.returns().give_back([3]);
        assert_eq!(s.in_use(), start + 1);
    }
}
