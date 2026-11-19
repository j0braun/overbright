//! Failed loads remembered (clone-side; no native counterpart): a load
//! that a frame path asks for again every frame (the garage platform and
//! FE car, the mix map, the minimap's TRACKMAP texture) is tried once per
//! request. The original loads each of these once, when its context is
//! built (a garage visit, a race, a race kind), and shows nothing when the
//! load fails; the clone asks from the frame, so without this a missing or
//! broken file is read from disk, and logged, on every frame.
//!
//! A failure is kept by what was asked for (a path, a car key, a track
//! number). The same request is not tried again until the request changes
//! (a key not remembered is tried at once) or the owner forgets the
//! failures because its context was built again (a new garage visit, a new
//! audio state); a load that succeeds forgets them too (the request
//! changed and the files read). Each failure is logged once, when it is
//! remembered. Loads that succeed are untouched: they go through as before.

/// The failed requests of one loader, newest first, at most `cap` of them
/// (a loader with one request at a time keeps one: changing the request
/// drops the old failure, so returning to it tries it again).
#[derive(Clone, Debug)]
pub struct FailedLoads<K> {
    failed: Vec<K>,
    cap: usize,
    /// Loads tried through [`FailedLoads::load`] or noted with
    /// [`FailedLoads::tried`] (a count for tests and the log).
    attempts: u32,
}

impl<K: PartialEq> Default for FailedLoads<K> {
    fn default() -> Self {
        Self::new(1)
    }
}

impl<K: PartialEq> FailedLoads<K> {
    /// Keeps up to `cap` (at least 1) failed requests.
    pub const fn new(cap: usize) -> Self {
        FailedLoads { failed: Vec::new(), cap: if cap == 0 { 1 } else { cap }, attempts: 0 }
    }

    /// Whether `key` failed and is still remembered: do not try it.
    pub fn remembers(&self, key: &K) -> bool {
        self.failed.contains(key)
    }

    /// A load of `key` is being tried (for loads not run through
    /// [`FailedLoads::load`], such as one on a thread).
    pub fn tried(&mut self) {
        self.attempts = self.attempts.wrapping_add(1);
    }

    /// `key` failed: remembered, and logged (once: a remembered key is not
    /// tried again, so it does not fail again).
    pub fn remember(&mut self, key: K, what: &str, why: impl std::fmt::Display) {
        if self.remembers(&key) {
            return;
        }
        log::warn!("{what}: {why:#} (not tried again until the request changes)");
        self.failed.insert(0, key);
        self.failed.truncate(self.cap);
    }

    /// `load` for `key` unless `key` failed before: its value (the
    /// failures kept forgotten: the request changed), or None (the failure
    /// remembered and logged once, or remembered already).
    pub fn load<T, E: std::fmt::Display>(
        &mut self,
        key: K,
        what: &str,
        load: impl FnOnce() -> Result<T, E>,
    ) -> Option<T> {
        if self.remembers(&key) {
            return None;
        }
        self.tried();
        match load() {
            Ok(v) => {
                self.forget();
                Some(v)
            }
            Err(e) => {
                self.remember(key, what, e);
                None
            }
        }
    }

    /// The context was built again: every request is tried again.
    pub fn forget(&mut self) {
        self.failed.clear();
    }

    /// The loads tried so far.
    pub fn attempts(&self) -> u32 {
        self.attempts
    }
}

#[cfg(test)]
mod tests {
    use super::FailedLoads;

    #[test]
    fn a_failed_request_is_tried_once_until_it_changes() {
        let mut f = FailedLoads::<&str>::new(1);
        let mut reads = 0;
        for _ in 0..100 {
            let got: Option<()> = f.load("A", "test", || {
                reads += 1;
                Err("missing")
            });
            assert!(got.is_none());
        }
        assert_eq!((reads, f.attempts()), (1, 1), "tried once across 100 frames");
        // Another request is tried at once; with one kept, A is tried again
        // after it.
        let _: Option<()> = f.load("B", "test", || Err("missing"));
        assert_eq!(f.attempts(), 2);
        let _: Option<()> = f.load("A", "test", || Err("missing"));
        assert_eq!(f.attempts(), 3);
        // A success is not remembered: the same request loads every time;
        // it forgets the failure (A fails once more after it).
        for _ in 0..3 {
            assert_eq!(f.load("C", "test", || Ok::<_, &str>(7)), Some(7));
        }
        assert_eq!(f.attempts(), 6);
        assert!(!f.remembers(&"A"));
        // A context built again forgets the failures.
        f.forget();
        let _: Option<()> = f.load("A", "test", || Err("missing"));
        assert_eq!(f.attempts(), 7);
    }

    #[test]
    fn several_kept_up_to_the_cap() {
        let mut f = FailedLoads::<u8>::new(2);
        f.remember(1, "test", "x");
        f.remember(2, "test", "x");
        assert!(f.remembers(&1) && f.remembers(&2));
        f.remember(3, "test", "x");
        assert!(!f.remembers(&1), "the oldest dropped");
        assert!(f.remembers(&2) && f.remembers(&3));
    }
}
