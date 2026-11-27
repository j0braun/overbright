//! The icon scrollers' scroll ease (0x574E40), shared by the icon menus and the parts browsers.

/// How long a scroll takes: both scroller constructors set the ease's
/// length to 0.3 (0x543AA0 at 0x543BDF for the screens' IconScrollerMenu,
/// 0x5435E0 for the parts browsers' strip and column).
pub(crate) const SCROLL_TIME: f32 = 300.0;

/// A scroller's offset ease (0x574E40): a cubic Hermite from the value and
/// speed when the target changed to the target at rest, over
/// [`SCROLL_TIME`]. A step (0x52EE50 / 0x538950) sets a new target and
/// starts again from where the scroll is (mode 2: the value and speed of
/// the moment carry over); filling a scroller (0x5386B0) puts it at the
/// target at once. Driven by time, so every frame rate sees the same curve.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollEase {
    /// Start time (ms), start value and speed (per ms), target.
    start: f32,
    p0: f32,
    v0: f32,
    p1: f32,
}

impl ScrollEase {
    /// At rest on `target` (0x5386B0).
    pub fn at(target: f32) -> ScrollEase {
        ScrollEase { start: 0.0, p0: target, v0: 0.0, p1: target }
    }

    /// At rest on `target` from `now` (an icon menu opened on an option).
    pub fn at_from(now: f32, target: f32) -> ScrollEase {
        ScrollEase { start: now, p0: target, v0: 0.0, p1: target }
    }

    /// The target it moves to.
    pub fn target(&self) -> f32 {
        self.p1
    }

    /// The value and its speed (per ms) at `now`.
    pub fn value(&self, now: f32) -> (f32, f32) {
        let t = ((now - self.start) / SCROLL_TIME).clamp(0.0, 1.0);
        if t >= 1.0 {
            return (self.p1, 0.0);
        }
        let v0 = self.v0 * SCROLL_TIME;
        let d = self.p1 - self.p0;
        let a = v0 - 2.0 * d;
        let b = 3.0 * d - 2.0 * v0;
        let value = ((a * t + b) * t + v0) * t + self.p0;
        let speed = ((3.0 * a * t + 2.0 * b) * t + v0) / SCROLL_TIME;
        (value, speed)
    }

    /// A new target from `now`, starting from the value and speed then
    /// (0x52EE50 sets mode 2 when the target differs from the value).
    pub fn retarget(&mut self, now: f32, target: f32) {
        let (p, v) = self.value(now);
        *self = ScrollEase { start: now, p0: p, v0: v, p1: target };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 0x574E40 with no start speed: half way at half the time, at rest
    /// on the target at the end; the same at any sampling rate.
    #[test]
    fn eases_by_time() {
        let mut e = ScrollEase::at(0.0);
        e.retarget(1000.0, -158.0);
        assert_eq!(e.value(1000.0).0, 0.0);
        assert!((e.value(1150.0).0 + 79.0).abs() < 1e-3);
        assert_eq!(e.value(1300.0), (-158.0, 0.0));
        // A step while moving keeps the speed (no jump in value).
        let before = e.value(1100.0).0;
        e.retarget(1100.0, -316.0);
        assert!((e.value(1100.0).0 - before).abs() < 1e-4);
        assert!(e.value(1100.0).1 < 0.0);
    }
}
