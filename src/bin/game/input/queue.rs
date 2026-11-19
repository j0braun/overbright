//! The window's key and mouse events on ticks.
//!
//! The original polls its input once per main-loop iteration, after the
//! frame clock (0x43BE20 → 0x57EAE0) and before the world update (0x5809D0
//! at 0x5811D0's step 13; measured under Wine: "the order
//! inside one main-loop iteration"). The clone's window delivers events
//! between drawn frames; they are queued here in their arrival order, each
//! with the tick it is due at, and applied at the start of that tick
//! (`Game::update`, before anything of the tick runs), so what a key or a
//! click does depends only on the tick, never on where in a drawn frame
//! it arrived or how fast the display runs.
//!
//! **The due tick** ([`due_tick`]): an event belongs to the tick whose
//! real-time window holds its arrival (tick k covers [k·T, (k+1)·T) of
//! the tick clock's accumulated time). At a steady 60 Hz that is the next
//! tick the next drawn frame runs — the original's "polled at the frame
//! start", no added latency. At 30 Hz a frame runs two ticks: an event that
//! came in the first half of the frame applies before the first of them,
//! one from the second half before the second (as at 60 Hz). At 120 / 200
//! Hz a frame often runs no tick: the event waits for the next one.
//!
//! **The injection point** ([`Game::input_at`]): the window path and the
//! developer harness both queue through it; the harness names the tick.

use std::collections::VecDeque;

use glam::Vec2;
use nfsu2::frame_clock::Rate;
use winit::keyboard::KeyCode;

use crate::*;

/// One input event as the tick applies it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum InputEvent {
    /// A key pressed (`repeat`: the system's auto-repeat, which only types
    /// text; `text`: the characters it typed, for the name editor and the
    /// splash screen's cheat input).
    KeyDown { code: KeyCode, text: Option<String>, repeat: bool },
    /// A key let go.
    KeyUp(KeyCode),
    /// The cursor at a window point (target pixels).
    CursorMoved(Vec2),
    /// The clicking button (`clicking_button`) pressed (true) or let go.
    Button(bool),
    /// The wheel, in lines (> 0 away from the player).
    Wheel(f32),
}

impl InputEvent {
    /// A key's press as the harness sends it (no text, not a repeat; the
    /// harness unit's flows move onto [`Game::input_at`] with it).
    #[allow(dead_code)]
    pub(crate) fn press(code: KeyCode) -> InputEvent {
        InputEvent::KeyDown { code, text: None, repeat: false }
    }
}

/// The queued events, in arrival order, each with its due tick.
#[derive(Clone, Debug, Default)]
pub(crate) struct InputQueue {
    events: VecDeque<(u64, InputEvent)>,
}

impl InputQueue {
    /// `event` to be applied at the start of tick `tick` (or of the first
    /// tick after it, when that one has run already).
    pub(crate) fn push(&mut self, tick: u64, event: InputEvent) {
        self.events.push_back((tick, event));
    }

    /// The events due at the start of tick `tick`, in the order they were
    /// queued; the later ones stay.
    pub(crate) fn take_due(&mut self, tick: u64) -> Vec<InputEvent> {
        if self.events.iter().all(|(t, _)| *t > tick) {
            return Vec::new();
        }
        let (due, later): (VecDeque<_>, VecDeque<_>) = self.events.drain(..).partition(|(t, _)| *t <= tick);
        self.events = later;
        due.into_iter().map(|(_, e)| e).collect()
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.events.len()
    }
}

/// The tick an event arriving now is due at: `next` is the next tick to
/// run, `acc` the tick clock's accumulator after the last drawn frame
/// (`TickClock::acc`, units of 1/36 000 s), `since` the units from that
/// frame's start to the arrival, `rate` the rate the ticks run at. The
/// whole periods that had passed by the arrival are ticks before it, at
/// most a frame's cap of them: when the frame then drops the time beyond
/// its cap (a stall), the event goes with the next frame's first tick.
pub(crate) fn due_tick(next: u64, acc: i64, since: i64, rate: Rate) -> u64 {
    let passed = (acc + since.max(0)).max(0) / rate.period;
    next + passed.min(i64::from(rate.cap)) as u64
}

impl Game {
    /// **The injection point** (window and harness): `event` applied at the
    /// start of tick `tick` (`GameClock::tick`, the index of the update it
    /// goes before; a tick already run means the next one).
    pub(crate) fn input_at(&mut self, tick: u64, event: InputEvent) {
        self.platform.input.push(tick, event);
    }

    /// A window event arriving now: queued for the tick whose real time
    /// holds its arrival ([`due_tick`]).
    pub(crate) fn input_now(&mut self, event: InputEvent) {
        let since = Instant::now().saturating_duration_since(self.pacing.last);
        let since = i64::try_from(since.as_nanos() * 9 / 250_000).unwrap_or(i64::MAX);
        let rate = if self.scene_frame_cap() { nfsu2::frame_clock::SCENE } else { nfsu2::frame_clock::RACE };
        let tick = due_tick(self.pacing.clocks.tick, self.pacing.cadence.acc, since, rate);
        self.input_at(tick, event);
    }

    /// The start of a tick: the events due at it, applied in their order
    /// (`Game::update`, first).
    pub(crate) fn apply_input_queue(&mut self) {
        for event in self.platform.input.take_due(self.pacing.clocks.tick) {
            self.apply_input(event);
        }
    }

    /// One queued event (what the window's handler once did on arrival,
    /// before input moved onto ticks).
    fn apply_input(&mut self, event: InputEvent) {
        match event {
            InputEvent::KeyDown { code, text, repeat } => self.key_down(code, text.as_deref(), repeat),
            InputEvent::KeyUp(code) => {
                self.platform.keys.remove(&code);
                self.key_released(code);
            }
            InputEvent::CursorMoved(_) | InputEvent::Button(_) | InputEvent::Wheel(_) => {
                // The texture editor's panel takes its clicks and wheel.
                if crate::enh::texed::mouse(self, &event) {
                    return;
                }
                let now = self.now();
                // A move's real-time stamp (0x50B492 stores 0x8651A8 in
                // 0x83A9CC when the cursor is not where it was drawn).
                let moved = matches!(event, InputEvent::CursorMoved(p) if self.platform.mouse.at != Some(p));
                crate::input::mouse::apply_mouse(&mut self.platform.mouse, &mut self.platform.mouse_state, &event, now);
                if moved {
                    self.platform.mouse.moved_real = self.real_now();
                }
            }
        }
    }

    /// A key pressed: the typed text to the screens that take it, the key
    /// itself (not its auto-repeats) to the game, then held.
    fn key_down(&mut self, code: KeyCode, text: Option<&str>, repeat: bool) {
        // The texture editor (`--editor` only; docs/modding/texture-editor.md).
        if crate::enh::texed::key(code, text, repeat) {
            return;
        }
        nfsu2::pad::extras::set_prompt(None);
        // MC_Main's name editor takes the typed text (0x551280); Backspace
        // edits there.
        if let Mode::Menu(Menu::McMain(m)) = &mut self.mode
            && m.step == McStep::Name
        {
            if code == KeyCode::Backspace {
                m.char('\u{8}');
                return;
            }
            if let Some(text) = text {
                text.chars().for_each(|c| m.char(c));
            }
        }
        // UG_LS_Splash: WM_CHAR (0x5CCD7B) goes to the cheat codes' input
        // (0x55DBE0 → 0x57B8C0).
        if matches!(self.mode, Mode::Menu(Menu::Splash(_)))
            && let Some(text) = text
        {
            text.chars().for_each(|c| self.cheat_char(c));
        }
        if !repeat && let Err(e) = self.key(code, &mut || {}) {
            eprintln!("{e:#}");
            self.mode = self.dialog(&format!("{e}"), self.icon_menu(MenuKind::Main));
        }
        self.platform.keys.insert(code);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nfsu2::frame_clock::{RACE, TickClock, UNITS};

    /// A display schedule's frame lengths (units) covering `secs`: exact
    /// at `hz`, or jittered (4–40 ms, a 120 ms stall now and then).
    fn schedule(hz: Option<u32>, secs: f64, seed: u64) -> Vec<i64> {
        let total = (secs * UNITS as f64) as i64;
        let mut v = Vec::new();
        let mut sum = 0;
        let mut s = seed;
        while sum < total {
            let u = match hz {
                Some(hz) => UNITS / i64::from(hz),
                None => {
                    s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    let r = (s >> 33) % 1000;
                    if r < 4 { 120 * 36 } else { 4 * 36 + ((s >> 20) % (36 * 36)) as i64 }
                }
            };
            v.push(u);
            sum += u;
        }
        v
    }

    /// The tick each event (arrival times in units since boot) is applied
    /// at when the window delivers it between the drawn frames of
    /// `frames`: queued with [`due_tick`] at its arrival, taken at the
    /// start of each tick the frames run.
    fn applied_ticks(frames: &[i64], arrivals: &[i64]) -> Vec<Option<u64>> {
        let mut clock = TickClock::default();
        let mut q = InputQueue::default();
        let mut out = vec![None; arrivals.len()];
        // Delivered in arrival order.
        let mut order: Vec<usize> = (0..arrivals.len()).collect();
        order.sort_by_key(|&i| arrivals[i]);
        let (mut next_tick, mut frame_start, mut a) = (0u64, 0i64, 0usize);
        for &units in frames {
            let end = frame_start + units;
            // Events delivered before this frame starts (they arrived in
            // the previous frame's time).
            while a < order.len() && arrivals[order[a]] < end {
                let i = order[a];
                let since = arrivals[i] - frame_start;
                q.push(due_tick(next_tick, clock.acc, since, RACE), InputEvent::Wheel(i as f32));
                a += 1;
            }
            let mut f = clock.begin(units);
            while clock.tick(&mut f, RACE).is_some() {
                for e in q.take_due(next_tick) {
                    let InputEvent::Wheel(i) = e else { unreachable!() };
                    out[i as usize] = Some(next_tick);
                }
                next_tick += 1;
            }
            frame_start = end;
        }
        out
    }

    /// I1: an event arriving anywhere has its effect at the same tick at
    /// 30, 60, 120 and 200 Hz: the tick whose 1/60 s window holds it.
    #[test]
    fn an_event_applies_at_the_same_tick_at_every_rate() {
        let period = RACE.period;
        // Arrivals all over the ticks' windows (not on a boundary, where
        // the frame and the event come at the same instant).
        let arrivals: Vec<i64> = (0..400).map(|i| period * (i / 7 + 1) + 1 + (i * 37) % (period - 2)).collect();
        let at60 = applied_ticks(&schedule(Some(60), 12.0, 0), &arrivals);
        for (i, t) in at60.iter().enumerate() {
            assert_eq!(*t, Some((arrivals[i] / period) as u64), "60 Hz, event {i}");
        }
        for hz in [30, 120, 200] {
            assert_eq!(applied_ticks(&schedule(Some(hz), 12.0, 0), &arrivals), at60, "{hz} Hz");
        }
    }

    /// I1 under a jittered display (4–40 ms frames, stalls): the same
    /// tick, except an event in the last 2 ms of a window whose tick a
    /// frame took early (the clock's snap, `SNAP_UNITS`): that tick had run
    /// when it came, so it goes with the next one; and after a stall past
    /// the 3-tick cap, whose dropped time has no ticks (the game slows
    /// down there, as the original's 3/60 s cap makes it): tick time and
    /// real time part there, so the comparison stops at the first stall.
    #[test]
    fn a_jittered_display_applies_events_at_their_tick() {
        let period = RACE.period;
        let arrivals: Vec<i64> = (0..3000).map(|i| period * (i / 5 + 1) + 1 + (i * 53) % (period - 2)).collect();
        for seed in [1, 2, 3] {
            let frames = schedule(None, 12.0, seed);
            // The schedule's real time against tick time: the dropped time
            // shifts every later window; compare until the first drop.
            let mut clock = TickClock::default();
            let (mut t, mut first_drop) = (0i64, i64::MAX);
            for &u in &frames {
                let mut f = clock.begin(u);
                while clock.tick(&mut f, RACE).is_some() {}
                t += u;
                if f.dropped > 0 {
                    first_drop = first_drop.min(t - u);
                }
            }
            let got = applied_ticks(&frames, &arrivals);
            let mut early = 0;
            for (i, &at) in arrivals.iter().enumerate().filter(|(_, at)| **at < first_drop) {
                let want = (at / period) as u64;
                let g = got[i].expect("applied");
                if g != want {
                    assert!(g == want + 1 && at % period >= period - nfsu2::frame_clock::SNAP_UNITS, "seed {seed}, event at {at}: tick {g}, want {want}");
                    early += 1;
                }
            }
            assert!(early * 50 < arrivals.len(), "seed {seed}: {early} snapped");
        }
    }

    /// Several events in one frame keep their order; at 200 Hz a frame
    /// without a tick keeps them for the next one.
    #[test]
    fn order_is_kept_and_events_wait_for_a_tick() {
        let mut q = InputQueue::default();
        q.push(3, InputEvent::press(KeyCode::ArrowDown));
        q.push(3, InputEvent::KeyUp(KeyCode::ArrowDown));
        q.push(4, InputEvent::press(KeyCode::Enter));
        q.push(3, InputEvent::Button(true));
        assert!(q.take_due(2).is_empty());
        assert_eq!(
            q.take_due(3),
            vec![InputEvent::press(KeyCode::ArrowDown), InputEvent::KeyUp(KeyCode::ArrowDown), InputEvent::Button(true)]
        );
        assert_eq!(q.len(), 1);
        // A tick that came late (the harness named a tick already run).
        assert_eq!(q.take_due(9), vec![InputEvent::press(KeyCode::Enter)]);
        // 200 Hz: 180 units a frame, a tick every 600: a frame without a
        // tick leaves the event for the next tick.
        let mut clock = TickClock::default();
        let mut f = clock.begin(180);
        assert!(clock.tick(&mut f, RACE).is_none());
        assert_eq!(due_tick(0, clock.acc, 50, RACE), 0);
    }

    /// A click acts at the point it arrived at, though the cursor moved on
    /// before the tick; hover then sees the newest point on the next tick.
    #[test]
    fn a_click_hits_where_it_arrived() {
        use crate::input::mouse::{MouseState, apply_mouse, sample_mouse};
        let (a, b, c) = (Vec2::new(100.0, 50.0), Vec2::new(300.0, 200.0), Vec2::new(310.0, 205.0));
        let mut m = nfsu2::frontend::mouse::Mouse::default();
        let mut st = MouseState::default();
        for e in [InputEvent::CursorMoved(a), InputEvent::Button(true), InputEvent::CursorMoved(b), InputEvent::Button(false), InputEvent::CursorMoved(c)] {
            apply_mouse(&mut m, &mut st, &e, 0.0);
        }
        let tick = sample_mouse(&mut m, &mut st);
        assert_eq!(tick.at, Some(a));
        assert!(tick.pressed && tick.released);
        // The next tick: a move from the click's point to the newest.
        let next = sample_mouse(&mut m, &mut st);
        assert_eq!(next.at, Some(c));
        assert!(next.moved() && !next.pressed);
        // Moves alone coalesce to the newest point.
        apply_mouse(&mut m, &mut st, &InputEvent::CursorMoved(a), 0.0);
        apply_mouse(&mut m, &mut st, &InputEvent::CursorMoved(b), 0.0);
        assert_eq!(sample_mouse(&mut m, &mut st).at, Some(b));
    }

    /// I2: the scroll arrows' repeat gate on the real-time clock fires on
    /// the same ticks at every display rate (a press each tick: one arrow
    /// step every 4th RACE tick, 0x538216).
    #[test]
    fn arrow_repeat_fires_on_the_same_ticks_at_every_rate() {
        use crate::input::mouse::arrow_gate_open;
        let fired = |frames: &[i64]| {
            let mut clock = TickClock::default();
            let mut game = nfsu2::game_clock::GameClock::default();
            let (mut last, mut out) = (None, Vec::new());
            for &u in frames {
                let mut f = clock.begin(u);
                while let Some(dt) = clock.tick(&mut f, RACE) {
                    game.begin();
                    if arrow_gate_open(game.real, last) {
                        last = Some(game.real);
                        out.push(game.tick);
                    }
                    game.tick(dt, true, false);
                }
            }
            out.truncate(100);
            out
        };
        let at60 = fired(&schedule(Some(60), 8.0, 0));
        assert_eq!(&at60[..4], &[0, 4, 8, 12]);
        for hz in [30, 120, 200] {
            assert_eq!(fired(&schedule(Some(hz), 8.0, 0)), at60, "{hz} Hz");
        }
    }

    /// A long frame: an event past the cap's ticks goes with the next
    /// frame's first tick (the stall's time beyond the cap is dropped).
    #[test]
    fn a_stall_puts_late_events_on_the_next_frames_first_tick() {
        assert_eq!(due_tick(10, 0, 5 * RACE.period + 3, RACE), 13);
        assert_eq!(due_tick(10, 0, 3 * RACE.period - 3, RACE), 12);
        assert_eq!(due_tick(10, -72, 10, RACE), 10);
        assert_eq!(due_tick(10, 0, RACE.period, RACE), 11);
    }
}
