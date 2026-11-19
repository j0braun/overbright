//! The system timer's resolution while the game runs (
//! docs/playing/platform-windows.md). Windows: timeBeginPeriod(1) for the guard's
//! lifetime, so `Sleep` and the waits behind `std::thread::sleep` wake
//! within about a millisecond instead of the default 15.6 ms tick (the
//! frame limiter's sleep, `display::wait_until`). Elsewhere a no-op (Linux
//! sleeps have sub-millisecond resolution already).

/// Holds the 1 ms timer resolution until dropped.
#[must_use = "the resolution is released when the guard is dropped"]
#[derive(Debug)]
pub struct TimerResolution {
    granted: bool,
}

impl TimerResolution {
    /// Asks for 1 ms (Windows); logs once whether it was granted.
    pub fn one_ms() -> TimerResolution {
        #[cfg(windows)]
        {
            let granted = super::win::timer_begin_1ms();
            log::info!("timer resolution 1 ms: {}", if granted { "granted" } else { "refused" });
            TimerResolution { granted }
        }
        #[cfg(not(windows))]
        {
            TimerResolution { granted: false }
        }
    }

    /// Whether the 1 ms resolution is held.
    pub fn granted(&self) -> bool {
        self.granted
    }
}

impl Drop for TimerResolution {
    fn drop(&mut self) {
        #[cfg(windows)]
        if self.granted {
            super::win::timer_end_1ms();
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn no_op_off_windows() {
        let t = super::TimerResolution::one_ms();
        assert_eq!(t.granted(), cfg!(windows));
    }
}
