use super::*;

/// Makes the maps of a world's slots on a background thread: results
/// arrive (slot, map or None) through [`Builder::poll`] as they are made.
/// [`Builder::wait`] blocks until all are in (shots, benches).
pub struct Builder<T = NormalMap> {
    rx: std::sync::mpsc::Receiver<(usize, Option<T>)>,
    pending: usize,
}

/// A job of [`Builder::spawn`]: makes one map (or none).
pub type Job<T> = Box<dyn FnOnce() -> Option<T> + Send>;

impl Builder<NormalMap> {
    /// Start on `jobs` (slot, texture, class, reflect, the texture pack's
    /// replacement if any); the textures are clones (their bytes shared,
    /// `cache::Bytes`).
    pub fn start(jobs: Vec<(usize, Texture, Class, bool, Option<PackSource>)>) -> Builder {
        Builder::spawn(
            jobs.into_iter()
                .map(|(slot, t, class, reflect, pack)| {
                    (slot, Box::new(move || map_for_source(&t, class, reflect, pack)) as Job<NormalMap>)
                })
                .collect(),
        )
    }
}

impl<T: Send + 'static> Builder<T> {
    /// Run `jobs` (slot, job) on a rayon pool from a background thread.
    pub fn spawn(jobs: Vec<(usize, Job<T>)>) -> Builder<T> {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut pending = jobs.len();
        let spawned = std::thread::Builder::new().name("surface maps".into()).spawn(move || {
            use rayon::prelude::*;
            jobs.into_par_iter().for_each_with(tx, |tx, (slot, job)| {
                // A panicking job (a bad pack texture) is a slot without a
                // map, not a builder that never finishes.
                let made = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job)).unwrap_or_else(|_| {
                    log::warn!("surface maps: the job for slot {slot} panicked; no map");
                    None
                });
                let _ = tx.send((slot, made));
            });
        });
        // No thread (the process at its limit): no maps, as for slots that
        // get none, rather than a panic in the middle of a load.
        if let Err(e) = spawned {
            log::warn!("surface maps: no thread ({e}); the world goes without them");
            pending = 0;
        }
        Builder { rx, pending }
    }

    /// The results since the last call.
    pub fn poll(&mut self) -> Vec<(usize, Option<T>)> {
        let v: Vec<_> = self.rx.try_iter().collect();
        self.pending -= v.len().min(self.pending);
        v
    }

    /// Every remaining result (blocking).
    pub fn wait(&mut self) -> Vec<(usize, Option<T>)> {
        let mut v = Vec::new();
        while self.pending > 0 {
            match self.rx.recv() {
                Ok(m) => {
                    v.push(m);
                    self.pending -= 1;
                }
                Err(_) => break,
            }
        }
        v
    }

    pub fn done(&self) -> bool {
        self.pending == 0
    }
}

/// Shots, benches and the selftest want the whole set before the first
/// frame (deterministic images): set by the headless modes.
static BLOCKING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub fn set_blocking(on: bool) {
    BLOCKING.store(on, std::sync::atomic::Ordering::Relaxed);
}
pub fn blocking() -> bool {
    BLOCKING.load(std::sync::atomic::Ordering::Relaxed)
}

// ---------------------------------------------------------------------
// Cars (docs/surface-maps.md "Cars"). A car's textures are atlases:
// the skin holds the paint, the grille, the trim and the interior; the
// tyre texture the rim and the sidewall. So a car map is made per texture
// from the regions (UV triangles) of the draws whose light material has
// relief, each judged on its own pixels; everything else in the map is
// flat (paint, vinyls, decals, glass, lights, chrome: native).
