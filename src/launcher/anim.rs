//! Motion: tweens timed on the wall clock in milliseconds (never on
//! frames, so a slow or fast redraw rate changes only smoothness, never
//! duration), look A's motion values, and the Reduced motion switch.
//!
//! Reduced motion: every tween shows its end value at once, so nothing
//! slides or fades. Waits that are not motion keep their length: Play's
//! 150 ms before "Starting…" still passes, with the glow simply on.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Look A's motion values (milliseconds; distances in layout units).
pub mod spec {
    /// A page change: the slab's content slides in this far …
    pub const PAGE_SLIDE_U: f32 = 24.0;
    /// … over this long, easing out.
    pub const PAGE_SLIDE_MS: u32 = 200;
    /// Play's lime glow on press, before "Starting…" shows.
    pub const PLAY_GLOW_MS: u32 = 150;
    /// Focus rings jump; no tween.
    pub const FOCUS_MS: u32 = 0;
    /// While something moves the window redraws about this often.
    pub const FRAME_MS: u32 = 16;
}

static REDUCED: AtomicBool = AtomicBool::new(false);

/// Turns Reduced motion on or off (the Advanced settings row).
pub fn set_reduced(on: bool) {
    REDUCED.store(on, Ordering::Relaxed)
}

pub fn reduced() -> bool {
    REDUCED.load(Ordering::Relaxed)
}

/// Easing curves over 0..=1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ease {
    Linear,
    /// Fast start, soft landing: page slides, the glow.
    OutCubic,
}

impl Ease {
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Ease::Linear => t,
            Ease::OutCubic => 1.0 - (1.0 - t).powi(3),
        }
    }
}

/// A value moving from `from` to `to` over `ms` milliseconds from `start`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tween {
    pub from: f32,
    pub to: f32,
    pub start: Instant,
    pub ms: u32,
    pub ease: Ease,
}

impl Tween {
    /// A tween starting now.
    pub fn new(from: f32, to: f32, ms: u32, ease: Ease) -> Tween {
        Tween::starting(from, to, ms, ease, Instant::now())
    }

    /// A tween starting at `start` (tests and renders pass a fixed instant).
    pub fn starting(from: f32, to: f32, ms: u32, ease: Ease, start: Instant) -> Tween {
        Tween { from, to, start, ms, ease }
    }

    /// The value at `now` (the end value under Reduced motion).
    pub fn at(&self, now: Instant) -> f32 {
        self.at_with(now, reduced())
    }

    /// The value at `now` with Reduced motion given explicitly.
    pub fn at_with(&self, now: Instant, reduced: bool) -> f32 {
        if reduced {
            return self.to;
        }
        let k = self.ease.apply(self.progress(now));
        self.from + (self.to - self.from) * k
    }

    /// Progress 0..=1 before easing (ignores Reduced motion).
    pub fn progress(&self, now: Instant) -> f32 {
        if self.ms == 0 {
            return 1.0;
        }
        let e = now.saturating_duration_since(self.start).as_secs_f32() * 1000.0;
        (e / self.ms as f32).clamp(0.0, 1.0)
    }

    pub fn done(&self, now: Instant) -> bool {
        self.done_with(now, reduced())
    }

    pub fn done_with(&self, now: Instant, reduced: bool) -> bool {
        reduced || now >= self.end()
    }

    /// When the tween ends.
    pub fn end(&self) -> Instant {
        self.start + Duration::from_millis(u64::from(self.ms))
    }
}

/// The motion values one frame is drawn with.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Frame {
    /// The slab content's slide (px, signed; 0 at rest).
    pub slide_px: f32,
    /// Play's glow (0..1).
    pub play_glow: f32,
    /// The busy spinner's turn (0..1, one turn a second; 0 with Reduced
    /// motion).
    pub spin: f32,
}

/// The spinner's turn at `now` (wall-clock; one turn a second).
pub fn spin_at(now: Instant) -> f32 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    if reduced() {
        return 0.0;
    }
    let t0 = *START.get_or_init(|| now);
    (now.saturating_duration_since(t0).as_millis() % 1000) as f32 / 1000.0
}

/// The screen's running motions (the App owns one).
#[derive(Clone, Debug)]
pub struct Motion {
    /// 1.0 at rest; rises to 1.0 after a page change.
    slide: Option<Tween>,
    /// +1: the new page comes in from the right (next), -1 from the left.
    slide_dir: f32,
    glow: Option<Tween>,
    /// The layout unit in pixels (the slide is 24 u).
    unit: f32,
}

impl Default for Motion {
    fn default() -> Self {
        Motion { slide: None, slide_dir: 1.0, glow: None, unit: 1.0 }
    }
}

impl Motion {
    /// The layout unit in pixels; set whenever the layout's size changes.
    pub fn set_unit(&mut self, u: f32) {
        self.unit = u;
    }

    /// A page change (`dir` −1 to the previous page, +1 to the next): the
    /// new content slides in from that side.
    pub fn page_changed(&mut self, dir: i32) {
        self.page_changed_at(dir, Instant::now());
    }

    pub fn page_changed_at(&mut self, dir: i32, now: Instant) {
        self.slide_dir = if dir < 0 { -1.0 } else { 1.0 };
        self.slide = Some(Tween::starting(0.0, 1.0, spec::PAGE_SLIDE_MS, Ease::OutCubic, now));
    }

    /// Play pressed: the glow rises over 150 ms; [`Motion::play_ready`]
    /// says when "Starting…" may show.
    pub fn play_pressed(&mut self) {
        self.play_pressed_at(Instant::now());
    }

    pub fn play_pressed_at(&mut self, now: Instant) {
        self.glow = Some(Tween::starting(0.0, 1.0, spec::PLAY_GLOW_MS, Ease::OutCubic, now));
    }

    /// Play's press has had its 150 ms (true when Play was not pressed).
    /// Not shortened by Reduced motion.
    pub fn play_ready(&self, now: Instant) -> bool {
        self.glow.is_none_or(|t| now >= t.end())
    }

    /// Clears Play's glow (when the game started or the start failed).
    pub fn play_reset(&mut self) {
        self.glow = None;
    }

    pub fn frame(&self, now: Instant) -> Frame {
        self.frame_with(now, reduced())
    }

    pub fn frame_with(&self, now: Instant, reduced: bool) -> Frame {
        let k = self.slide.map_or(1.0, |t| t.at_with(now, reduced));
        Frame {
            slide_px: self.slide_dir * spec::PAGE_SLIDE_U * self.unit * (1.0 - k),
            play_glow: self.glow.map_or(0.0, |t| t.at_with(now, reduced)),
            spin: if reduced { 0.0 } else { spin_at(now) },
        }
    }

    /// Something still moves or waits: keep redrawing.
    pub fn busy(&self, now: Instant) -> bool {
        self.busy_with(now, reduced())
    }

    pub fn busy_with(&self, now: Instant, reduced: bool) -> bool {
        self.slide.is_some_and(|t| !t.done_with(now, reduced)) || !self.play_ready(now)
    }

    /// When to draw next: `None` when nothing moves, else one frame from
    /// `now` or the last end, whichever is sooner (the last frame lands on
    /// the end value).
    pub fn next_frame(&self, now: Instant) -> Option<Instant> {
        if !self.busy(now) {
            return None;
        }
        let ends = [self.slide.map(|t| t.end()), self.glow.map(|t| t.end())];
        let last = ends.into_iter().flatten().filter(|e| *e > now).max()?;
        Some((now + Duration::from_millis(u64::from(spec::FRAME_MS))).min(last))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn eases_hit_their_ends_and_rise() {
        for e in [Ease::Linear, Ease::OutCubic] {
            assert_eq!(e.apply(0.0), 0.0);
            assert!((e.apply(1.0) - 1.0).abs() < 1e-6);
            assert_eq!(e.apply(-1.0), 0.0);
            assert!((e.apply(2.0) - 1.0).abs() < 1e-6);
            let mut prev = 0.0;
            for i in 1..=20 {
                let v = e.apply(i as f32 / 20.0);
                assert!(v >= prev, "{e:?} falls at {i}");
                prev = v;
            }
        }
        assert!(Ease::OutCubic.apply(0.25) > 0.25);
    }

    #[test]
    fn tween_follows_the_wall_clock_not_frames() {
        let t0 = Instant::now();
        let t = Tween::starting(10.0, 20.0, 200, Ease::Linear, t0);
        assert_eq!(t.at_with(t0, false), 10.0);
        assert!((t.at_with(t0 + ms(100), false) - 15.0).abs() < 1e-3);
        assert_eq!(t.at_with(t0 + ms(200), false), 20.0);
        assert_eq!(t.at_with(t0 + ms(5000), false), 20.0);
        assert!(!t.done_with(t0 + ms(199), false));
        assert!(t.done_with(t0 + ms(200), false));
        // Sampled often or once, the same instant gives the same value.
        let mut a = 0.0;
        for i in 0..=120 {
            a = t.at_with(t0 + ms(i), false);
        }
        assert_eq!(a, t.at_with(t0 + ms(120), false));
        assert_eq!(Tween::starting(1.0, 2.0, 0, Ease::Linear, t0).at_with(t0, false), 2.0);
    }

    #[test]
    fn reduced_motion_jumps_to_the_end_but_keeps_plays_wait() {
        let t0 = Instant::now();
        let t = Tween::starting(0.0, 1.0, 200, Ease::OutCubic, t0);
        assert_eq!(t.at_with(t0, true), 1.0);
        assert!(t.done_with(t0, true));
        let mut m = Motion::default();
        m.set_unit(1.5);
        m.page_changed_at(1, t0);
        m.play_pressed_at(t0);
        assert_eq!(m.frame_with(t0, true), Frame { slide_px: 0.0, play_glow: 1.0, spin: 0.0 });
        assert!(!m.play_ready(t0 + ms(149)));
        assert!(m.play_ready(t0 + ms(150)));
        assert!(m.busy_with(t0 + ms(100), true), "Play's wait still runs");
        assert!(!m.busy_with(t0 + ms(150), true));
    }

    #[test]
    fn page_slide_comes_from_the_side_of_the_new_page() {
        let t0 = Instant::now();
        let mut m = Motion::default();
        {
            let f = m.frame_with(t0, false);
            assert_eq!((f.slide_px, f.play_glow), (0.0, 0.0));
        }
        assert!(!m.busy_with(t0, false));
        m.set_unit(2.0);
        m.page_changed_at(1, t0);
        assert_eq!(m.frame_with(t0, false).slide_px, 48.0);
        m.set_unit(1.0);
        m.page_changed_at(-1, t0);
        assert_eq!(m.frame_with(t0, false).slide_px, -24.0);
        let mid = m.frame_with(t0 + ms(100), false).slide_px;
        assert!(mid > -24.0 * 0.25 && mid < 0.0, "ease-out is past half way: {mid}");
        assert_eq!(m.frame_with(t0 + ms(200), false).slide_px, 0.0);
        assert!(m.busy_with(t0 + ms(199), false));
        assert!(!m.busy_with(t0 + ms(200), false));
    }

    #[test]
    fn play_glow_rises_then_holds_until_reset() {
        let t0 = Instant::now();
        let mut m = Motion::default();
        assert!(m.play_ready(t0));
        m.play_pressed_at(t0);
        assert_eq!(m.frame_with(t0, false).play_glow, 0.0);
        let g = m.frame_with(t0 + ms(75), false).play_glow;
        assert!(g > 0.5 && g < 1.0, "{g}");
        assert_eq!(m.frame_with(t0 + ms(400), false).play_glow, 1.0);
        m.play_reset();
        assert_eq!(m.frame_with(t0 + ms(400), false).play_glow, 0.0);
        assert!(m.play_ready(t0));
    }

    #[test]
    fn global_switch_round_trips() {
        // Only this test touches the global; the others pass it explicitly.
        let was = reduced();
        set_reduced(true);
        assert!(reduced());
        let t0 = Instant::now();
        assert_eq!(Tween::starting(0.0, 5.0, 1000, Ease::Linear, t0).at(t0), 5.0);
        set_reduced(was);
        assert_eq!(spec::FOCUS_MS, 0);
    }
}
