//! Enhancement effects on ticks (docs/original/uncapped-fps.md, "Enhancement effects on ticks"): what
//! the enhanced effects that simulate, schedule or accumulate read of the
//! world's ticks, so their picture is the same at every display rate.
//! Clone-only (nothing here is in SPEED2.EXE); the Original preset never
//! reads it.
//!
//! * **Tick views** ([`TickView`]): one per world tick (the tick index
//!   `GameClock::tick`, the game clock's seconds at it, the tick's camera
//!   and the sections drawn from its eye). The game publishes, once per
//!   drawn frame and before the world is planned, the ticks taken since the
//!   last drawn frame ([`publish`], oldest first, at most [`MAX_VIEWS`]);
//!   a frame without a tick publishes none.
//! * **Tick-stepped effects** (a simulation or a schedule: LOD Fade's
//!   per-instance fades, the lamps' section fades and their shadow layers'
//!   hand-out, the Next-Gen water's basins, the traced scene's car cache,
//!   the light probes' bakes) keep a [`TickGate`] and step once per view
//!   it hands them, with that tick's inputs: at 30 Hz a frame steps two
//!   ticks, at 200 Hz most frames step none. Their state at tick n is then
//!   a function of the ticks alone.
//! * **History effects** (TAA, the Next-Gen atmosphere, the volumetric
//!   particles' temporal resolve) accumulate drawn frames by nature; they
//!   take the frame's place on the tick axis ([`frame_at`]: n − 1 + α) and
//!   scale their per-frame weight by the frame's span in ticks
//!   ([`history_weight`]): f' = 1 − (1 − f)^span, exactly f at span 1 (a
//!   steady 60 Hz), so the history's time constant is the same at every
//!   rate.
//!
//! Without a published frame (shots, the front end's scenes, a renderer
//! used outside the game's window path) every effect behaves as before:
//! one step per draw with the draw's own inputs ([`frame`] is None). The
//! garage's scene, drawn while a drive stays loaded under a shop or the
//! crib, sets the drive's frame aside for its draw ([`SetAside`]).
use crate::render::camera::Camera;
use std::sync::Mutex;

/// The most tick views a frame carries (the tick clock's cap is 3 a frame;
/// one spare).
pub const MAX_VIEWS: usize = 4;

/// The settings a tick stepped with that the tick-stepped effects read
/// (the game's tick settings, taken once per tick): a step for that tick
/// uses these, not the drawn frame's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TickOptions {
    /// LOD Fade.
    pub lod_fade: bool,
    /// World Detail Ultra / Extreme's pop-in fixes in force.
    pub pop_free: bool,
    /// Keep All Loaded (the sections drawn from the tick's eye).
    pub keep_loaded: bool,
    /// World Detail Extreme's far sections.
    pub far_draw: bool,
}

/// One world tick as the tick-stepped effects read it.
#[derive(Clone, Debug)]
pub struct TickView {
    /// The tick index (`GameClock::tick`: world updates since boot).
    pub tick: u64,
    /// The game clock's seconds at the tick (`Game::seconds`): the time
    /// the effects' fades move by.
    pub time: f32,
    /// The world clock (s, 0x7FB718) at the tick.
    pub clock: f32,
    /// The drive's camera as the tick left it (the frame's jitter copied
    /// on, so a tick drawn at α = 1 has the frame's camera bit for bit).
    pub camera: Camera,
    /// The sections drawn from the tick's eye (`World::drawn_sections_at`;
    /// None: all) and World Detail Extreme's far ones.
    pub sections: Option<std::sync::Arc<[u32]>>,
    pub far: Option<std::sync::Arc<[u32]>>,
    /// The settings the tick stepped with.
    pub opts: TickOptions,
}

impl TickView {
    /// Whether the tick is the frame's own view (the same eye and drawn
    /// sections: a frame drawn at α = 1), so an effect can step with the
    /// frame's inputs as they are (nothing computed twice).
    pub fn is_frame(&self, camera: &Camera, sections: Option<&[u32]>, far: Option<&[u32]>) -> bool {
        let bits = |c: &Camera| [c.position.x, c.position.y, c.position.z, c.yaw, c.pitch].map(f32::to_bits);
        bits(&self.camera) == bits(camera)
            && self.camera.up.to_array().map(f32::to_bits) == camera.up.to_array().map(f32::to_bits)
            && self.sections.as_deref() == sections
            && self.far.as_deref() == far
    }
}

/// A drawn frame's ticks.
#[derive(Clone, Debug, Default)]
pub struct TickFrame {
    /// The ticks taken since the last drawn frame, oldest first (empty: a
    /// frame between two ticks).
    pub views: Vec<TickView>,
    /// The frame's place on the tick axis: the newest tick's index less
    /// (1 − α) (a frame drawn at α = 1 stands on its tick).
    pub at: f64,
    /// How far the frame lies behind the newest tick on the world clock
    /// (s): (1 − α) · the tick's dt, 0 on a tick's own frame.
    pub lag: f32,
}

static FRAME: Mutex<Option<std::sync::Arc<TickFrame>>> = Mutex::new(None);
/// The newest tick published so far (a later frame publishes only newer).
static THROUGH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The game's drawn frame publishes its ticks (None: a frame drawn outside
/// the window path, every effect as before).
pub fn publish(frame: Option<TickFrame>) {
    if let Some(v) = frame.as_ref().and_then(|f| f.views.last()) {
        THROUGH.fetch_max(v.tick, std::sync::atomic::Ordering::Relaxed);
    }
    *FRAME.lock().unwrap() = frame.map(std::sync::Arc::new);
}

/// The published frame set aside while a scene of its own is drawn (the
/// front end's garage over a loaded drive): its renderers' tick-stepped
/// effects step per draw with the draw's inputs, as without a published
/// frame, instead of with the drive's ticks (whose camera and sections
/// are not the scene's: LOD Fade faded the whole platform out). Put back
/// when dropped.
pub struct SetAside(Option<std::sync::Arc<TickFrame>>);

impl SetAside {
    pub fn new() -> SetAside {
        SetAside(FRAME.lock().unwrap().take())
    }
}

impl Default for SetAside {
    fn default() -> Self {
        SetAside::new()
    }
}

impl Drop for SetAside {
    fn drop(&mut self) {
        let mut frame = FRAME.lock().unwrap_or_else(|e| e.into_inner());
        if frame.is_none() {
            *frame = self.0.take();
        }
    }
}

/// The world clock (s, 0x7FB718) at the frame's place
/// between the last two ticks: the newest tick's less the frame's lag
/// (exactly `clock` on a tick's own frame and without a published frame).
/// The rain's ripples, the wet cars' drops and the Next-Gen atmosphere's
/// noise move by it, as the water does.
pub fn drawn_clock(clock: f32) -> f32 {
    match FRAME.lock().unwrap().as_ref() {
        Some(f) if f.lag != 0.0 => clock - f.lag,
        _ => clock,
    }
}

/// A new tick clock (a new game in the same process: the tests): the
/// published mark starts over.
pub fn rewind() {
    THROUGH.store(0, std::sync::atomic::Ordering::Relaxed);
}

/// The newest tick a frame published (0: none yet).
pub fn published_through() -> u64 {
    THROUGH.load(std::sync::atomic::Ordering::Relaxed)
}

/// A tick as the game records it at its end (the sections are looked up
/// only for the ticks a frame publishes).
#[derive(Clone, Copy, Debug)]
pub struct TickRecord {
    pub tick: u64,
    pub time: f32,
    pub clock: f32,
    pub camera: Option<Camera>,
    /// The settings the tick stepped with.
    pub opts: TickOptions,
}

/// This frame's ticks, if the game published them.
pub fn frame() -> Option<std::sync::Arc<TickFrame>> {
    FRAME.lock().unwrap().clone()
}

/// This frame's place on the tick axis, if published.
pub fn frame_at() -> Option<f64> {
    FRAME.lock().unwrap().as_ref().map(|f| f.at)
}

/// The world renderer's tick-stepped enhancement state as a
/// digest (LOD Fade's per-instance fades, the lamps' section fades and
/// shadow layers, the traced scene's car cache): the rate tests compare it
/// at equal ticks across display rates.
pub fn digest(world: &crate::render::WorldRenderer) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    if let Some(f) = world.fade.as_ref() {
        f.digest(&mut h);
    }
    if let Some(f) = crate::render::enh::lighting::lamps::POP_FADES.lock().unwrap().as_ref() {
        let mut s: Vec<(u32, u32)> = f.sections.iter().map(|(k, v)| (*k, v.to_bits())).collect();
        s.sort_unstable();
        s.hash(&mut h);
    }
    if let Some(m) = world.lamp.as_ref().and_then(|l| l.maps.as_ref()) {
        m.holders.hash(&mut h);
    }
    if let Some(m) = crate::render::enh::lighting::lamps::POP_SHADOWS.lock().unwrap().as_ref() {
        let mut s: Vec<(usize, u32)> = m.iter().map(|(k, v)| (*k, v.to_bits())).collect();
        s.sort_unstable();
        s.hash(&mut h);
    }
    crate::render::enh::lighting::rt::digest(&mut h);
    h.finish()
}

/// What a tick-stepped effect steps this frame.
#[derive(Clone, Debug)]
pub enum Due {
    /// No published frame: one step with the draw's own inputs (as before).
    Draw,
    /// These ticks, oldest first (empty: none this frame).
    Ticks(Vec<TickView>),
}

/// A tick-stepped effect's last tick.
#[derive(Clone, Copy, Debug, Default)]
pub struct TickGate {
    last: Option<u64>,
}

impl TickGate {
    /// The ticks of `frame` this effect has not stepped yet (all of them
    /// after a reset), marked stepped.
    pub fn due(&mut self, frame: Option<&TickFrame>) -> Due {
        let Some(f) = frame else {
            self.last = None;
            return Due::Draw;
        };
        let out: Vec<TickView> = f.views.iter().filter(|v| self.last.is_none_or(|l| v.tick > l)).cloned().collect();
        if let Some(v) = out.last() {
            self.last = Some(v.tick);
        }
        Due::Ticks(out)
    }

    /// Forget the last tick (the effect's state was dropped).
    pub fn reset(&mut self) {
        self.last = None;
    }

    /// The last tick stepped.
    pub fn last(&self) -> Option<u64> {
        self.last
    }
}

/// A history effect's last drawn frame on the tick axis.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameSpan {
    last: Option<f64>,
}

impl FrameSpan {
    /// This frame's span since the last one (in ticks; 1 without a
    /// published frame or a last one, for a frame at the last one's place,
    /// and for any span within 1/1000 of a tick of 1, so a steady 60 Hz is
    /// exactly the old per-frame path).
    pub fn span(&mut self, at: Option<f64>) -> f32 {
        let Some(at) = at else {
            self.last = None;
            return 1.0;
        };
        // A frame at the same place as the last (a redraw of one instant:
        // the harness's still frames, a shot's warm-up) counts as a whole
        // frame, as before.
        let span = match self.last {
            Some(l) if at > l => (at - l) as f32,
            _ => 1.0,
        };
        self.last = Some(at);
        if (span - 1.0).abs() < 1e-3 { 1.0 } else { span.clamp(0.0, 8.0) }
    }

    /// The history restarts.
    pub fn reset(&mut self) {
        self.last = None;
    }
}

/// A per-frame history weight `w` (the share of the new frame) for a
/// frame spanning `span` ticks: the share the old frames keep decays as
/// (1 − w)^span, so the history's time constant in ticks is the same at
/// every rate. Exactly `w` at span 1; 0 at span 0 (a frame that adds no
/// time adds no weight).
pub fn history_weight(w: f32, span: f32) -> f32 {
    if span == 1.0 {
        return w;
    }
    (1.0 - (1.0 - w.clamp(0.0, 1.0)).powf(span)).clamp(0.0, 1.0)
}

/// The share an old history keeps (`h` per frame) over `span` ticks:
/// h^span, exactly `h` at span 1.
pub fn history_keep(h: f32, span: f32) -> f32 {
    if span == 1.0 {
        return h;
    }
    h.clamp(0.0, 1.0).powf(span)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    fn view(tick: u64) -> TickView {
        TickView {
            tick,
            time: tick as f32 / 60.0,
            clock: tick as f32 / 60.0,
            camera: Camera::new(Vec3::ZERO, 0.0, 0.0),
            sections: None,
            far: None,
            opts: Default::default(),
        }
    }

    /// The display schedules' frames: per frame the ticks taken (the tick
    /// clock's integer accumulator in 1/36 000 s, a tick every 600 units,
    /// at most 3 a frame) and α.
    pub(crate) fn schedule(hz: u64, ticks: u64) -> Vec<(Vec<u64>, f32)> {
        let unit = 36_000 / hz;
        let (mut acc, mut n, mut out) = (0u64, 0u64, Vec::new());
        while n < ticks {
            acc += unit;
            let mut taken = Vec::new();
            while acc >= 600 && taken.len() < 3 {
                acc -= 600;
                n += 1;
                taken.push(n);
            }
            out.push((taken, acc as f32 / 600.0));
        }
        out
    }

    /// A gate hands each tick out once, in order, whatever the frames
    /// group them by; without a published frame it falls back to the draw.
    #[test]
    fn a_gate_steps_every_tick_once() {
        for hz in [30, 60, 120, 200] {
            let mut g = TickGate::default();
            let mut seen = Vec::new();
            for (taken, _) in schedule(hz, 120) {
                let f = TickFrame { views: taken.iter().map(|&t| view(t)).collect(), at: 0.0, lag: 0.0 };
                match g.due(Some(&f)) {
                    Due::Ticks(v) => seen.extend(v.iter().map(|v| v.tick)),
                    Due::Draw => unreachable!(),
                }
            }
            assert_eq!(seen, (1..=120).collect::<Vec<_>>(), "{hz} Hz");
        }
        let mut g = TickGate::default();
        assert!(matches!(g.due(None), Due::Draw));
        assert_eq!(g.last(), None);
    }

    /// A history's weight over a frame's span: exactly the per-frame value
    /// at span 1, and two frames of half a tick decay as much as one of a
    /// whole tick (the time constant is the same at 60 and 120 Hz).
    #[test]
    fn history_weights_keep_their_time_constant() {
        for w in [0.1f32, 0.08, 0.12, 0.15] {
            assert_eq!(history_weight(w, 1.0).to_bits(), w.to_bits());
            let half = history_weight(w, 0.5);
            let two = 1.0 - (1.0 - half) * (1.0 - half);
            assert!((two - w).abs() < 1e-6, "{w}: {two}");
            let k = history_keep(1.0 - w, 0.3);
            assert!((k.powf(1.0 / 0.3) - (1.0 - w)).abs() < 1e-5);
        }
        assert_eq!(history_weight(0.1, 0.0), 0.0);
        let mut s = FrameSpan::default();
        assert_eq!(s.span(Some(10.0)), 1.0);
        assert_eq!(s.span(Some(11.00001)), 1.0);
        assert!((s.span(Some(11.5)) - 0.5).abs() < 1e-4);
        assert_eq!(s.span(Some(11.5)), 1.0, "a redraw of the same instant");
        assert_eq!(s.span(None), 1.0);
    }
}
