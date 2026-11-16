//! The stable id a simulated car (or the scene crowd) keeps in place
//! of its GPU objects. The renderers live in the game's render state
//! (`render::state::RenderState`), keyed by this id; the sim holds no wgpu
//! object.

/// A sim object's draw id: unique for the run (never reused), not `Clone`
/// (one car, one id). The render state watches it: once the car that holds
/// it is dropped, its renderer goes at the render state's next sweep.
#[derive(Debug)]
pub(crate) struct DrawId(std::sync::Arc<u64>);

/// A car's draw id (`RenderState.cars[CarId]`).
pub(crate) type CarId = DrawId;

impl DrawId {
    pub(crate) fn new() -> DrawId {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        DrawId(std::sync::Arc::new(NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)))
    }

    /// The id's key in the render state's maps.
    pub(crate) fn key(&self) -> u64 {
        *self.0
    }

    /// A handle that tells whether the id (its car) is still alive.
    pub(crate) fn watch(&self) -> std::sync::Weak<u64> {
        std::sync::Arc::downgrade(&self.0)
    }
}
