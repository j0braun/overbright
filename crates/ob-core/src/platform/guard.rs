//! Whether the calling thread is inside a guarded call into the window system
//! (`platform::monitor::guarded` in the library): the panic hooks say nothing
//! then, the caller logs one warning and goes on.

thread_local! {
    /// Inside a guarded call: the panic hook stays quiet (no message, no
    /// crash report).
    static GUARDED: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// Whether the calling thread is inside a guarded call (the panic hooks
/// say nothing then).
pub fn in_guard() -> bool {
    GUARDED.with(|g| g.get() > 0)
}

/// Runs `f` as a guarded call: [`in_guard`] is true inside it.
pub fn enter<T>(f: impl FnOnce() -> T) -> T {
    GUARDED.with(|g| g.set(g.get() + 1));
    let r = f();
    GUARDED.with(|g| g.set(g.get() - 1));
    r
}
