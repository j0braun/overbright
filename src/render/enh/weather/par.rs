//! PERF-RAIN (Phase 6): element-wise steps of the weather effects spread
//! over the render job pool ([`crate::render::Pending`]) and JOINED before
//! the call returns.
//!
//! [`chunks_mut`] cuts a slice into fixed chunks; the calling thread and up
//! to [`HELPERS`] pool jobs take chunks from one counter until none is
//! left, then the caller waits for every job that is inside a chunk. A job
//! the pool starts later (its workers busy with a frame's encoders) finds
//! the call closed and returns without touching the slice, so the caller
//! never waits for a job to start. Each element is worked by exactly one
//! thread with the same function, so the result is the serial loop's bit
//! for bit whatever the threads, the chunking or the timing (the callers'
//! functions read shared inputs only and write their own element).

use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};

/// Pool jobs besides the calling thread at most (the pool has four workers
/// at most; one is left for a frame's submit / present).
pub const HELPERS: usize = 3;

/// The pool jobs a call uses besides the calling thread: [`HELPERS`], or
/// `NFSU2_WEATHER_HELPERS` (0: serial). Read once.
pub fn helpers() -> usize {
    static N: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *N.get_or_init(|| crate::render::debug_env::nfsu2_weather_helpers().map_or(HELPERS, |n| n.min(HELPERS)))
}

/// The caller's chunk work, type-erased: the context and its trampoline.
struct Shared {
    ctx: *const (),
    call: unsafe fn(*const (), usize),
    chunks: usize,
    next: AtomicUsize,
    gate: Mutex<Gate>,
    done: Condvar,
}

#[derive(Default)]
struct Gate {
    /// The caller has finished: a job starting now does nothing.
    closed: bool,
    /// Jobs inside the chunk loop.
    active: usize,
    panicked: bool,
}

// SAFETY: `ctx` / `call` are used only by jobs registered in `gate.active`
// while the call is open; the caller (which owns what `ctx` points at)
// waits for `active == 0` after closing, before it returns or unwinds.
// What `ctx` points at is `Ctx<T, F>` with `T: Send` and `F: Sync`.
unsafe impl Send for Shared {}
unsafe impl Sync for Shared {}

impl Shared {
    fn drain(&self) {
        loop {
            let i = self.next.fetch_add(1, Ordering::Relaxed);
            if i >= self.chunks {
                return;
            }
            // SAFETY: chunk `i` is claimed by this thread alone (the
            // counter), and the call is open (see `Shared`'s note).
            unsafe { (self.call)(self.ctx, i) };
        }
    }
}

struct Ctx<'a, T, F> {
    ptr: *mut T,
    len: usize,
    chunk: usize,
    f: &'a F,
}

/// # Safety
/// `ctx` is a live `Ctx<T, F>` and chunk `i` is claimed by the caller alone.
unsafe fn run<T, F: Fn(usize, &mut [T])>(ctx: *const (), i: usize) {
    // SAFETY: as the function's contract.
    let c = unsafe { &*(ctx as *const Ctx<'_, T, F>) };
    let start = i * c.chunk;
    let end = (start + c.chunk).min(c.len);
    // SAFETY: chunks are disjoint ranges of the caller's slice.
    let part = unsafe { std::slice::from_raw_parts_mut(c.ptr.add(start), end - start) };
    (c.f)(start, part);
}

/// `f(start, chunk)` over `items` in chunks of `chunk` elements (`start`
/// the chunk's first index), on this thread and the job pool; returns
/// once every chunk is done. A slice of one chunk or less runs here alone.
/// A panic in `f` (on any thread) is raised here after the join.
pub fn chunks_mut<T: Send, F: Fn(usize, &mut [T]) + Sync>(items: &mut [T], chunk: usize, f: F) {
    chunks_mut_with(items, chunk, helpers(), f);
}

/// [`chunks_mut`] with `helpers` pool jobs besides this thread (0: serial).
fn chunks_mut_with<T: Send, F: Fn(usize, &mut [T]) + Sync>(items: &mut [T], chunk: usize, helpers: usize, f: F) {
    let chunk = chunk.max(1);
    let len = items.len();
    if len <= chunk || helpers == 0 {
        // Serial: the chunks in order on this thread (each element alone, so
        // the same values as any split).
        for (i, part) in items.chunks_mut(chunk).enumerate() {
            f(i * chunk, part);
        }
        return;
    }
    let chunks = len.div_ceil(chunk);
    let ctx = Ctx { ptr: items.as_mut_ptr(), len, chunk, f: &f };
    let shared = Arc::new(Shared {
        ctx: (&raw const ctx).cast(),
        call: run::<T, F>,
        chunks,
        next: AtomicUsize::new(0),
        gate: Mutex::new(Gate::default()),
        done: Condvar::new(),
    });
    for _ in 0..helpers.min(chunks - 1) {
        let sh = shared.clone();
        // The value is not waited for: the join is the gate below.
        drop(crate::render::Pending::spawn(move || {
            {
                let mut g = sh.gate.lock().unwrap();
                if g.closed {
                    return;
                }
                g.active += 1;
            }
            let r = catch_unwind(AssertUnwindSafe(|| sh.drain()));
            let mut g = sh.gate.lock().unwrap();
            g.active -= 1;
            g.panicked |= r.is_err();
            sh.done.notify_all();
        }));
    }
    let mine = catch_unwind(AssertUnwindSafe(|| shared.drain()));
    let mut g = shared.gate.lock().unwrap();
    g.closed = true;
    while g.active > 0 {
        g = shared.done.wait(g).unwrap();
    }
    let panicked = g.panicked;
    drop(g);
    if let Err(e) = mine {
        resume_unwind(e);
    }
    assert!(!panicked, "a weather job panicked");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every element worked once, in place, whatever the chunking; the
    /// result equals the serial loop's.
    #[test]
    fn chunks_cover_every_element_once() {
        for (n, chunk) in [(0usize, 4usize), (3, 4), (4, 4), (5, 4), (1000, 7), (10_000, 512)] {
            let mut v: Vec<u64> = (0..n as u64).collect();
            chunks_mut(&mut v, chunk, |start, part| {
                for (k, x) in part.iter_mut().enumerate() {
                    assert_eq!(*x, (start + k) as u64);
                    *x = x.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 1;
                }
            });
            let want: Vec<u64> = (0..n as u64).map(|x| x.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 1).collect();
            assert_eq!(v, want, "n {n} chunk {chunk}");
        }
    }

    /// Serial (no helpers) and any number of helpers give the same values.
    #[test]
    fn serial_and_parallel_give_the_same_values() {
        let work = |start: usize, part: &mut [f32]| {
            for (k, x) in part.iter_mut().enumerate() {
                *x = ((start + k) as f32 * 0.37).sin().mul_add(*x, 1.5).sqrt();
            }
        };
        let fresh = || (0..10_000).map(|i| i as f32 * 0.01).collect::<Vec<f32>>();
        let mut serial = fresh();
        chunks_mut_with(&mut serial, 512, 0, work);
        for helpers in 1..=HELPERS {
            let mut v = fresh();
            chunks_mut_with(&mut v, 512, helpers, work);
            assert!(v.iter().zip(&serial).all(|(a, b)| a.to_bits() == b.to_bits()), "helpers {helpers}");
        }
    }

    /// A panic in a chunk reaches the caller (after the join).
    #[test]
    fn a_panic_reaches_the_caller() {
        let mut v = vec![0u32; 4096];
        let r = catch_unwind(AssertUnwindSafe(|| {
            chunks_mut(&mut v, 64, |start, _| {
                if start == 64 * 40 {
                    panic!("chunk 40");
                }
            })
        }));
        assert!(r.is_err());
    }
}
