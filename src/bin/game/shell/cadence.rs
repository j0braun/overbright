//! The fixed-rate updates against the display's frames: `advance`, the
//! interpolation between the last two updates and the scenes' frame cap.
use crate::*;

impl Game {
    /// The world's part of a drawn frame `interval` seconds long: the
    /// tick clock's ticks stepped; how far the drawing is between the last
    /// two states (`draw_between`'s alpha). The harness and the tests call
    /// it; the window measures its frame in nanoseconds (`advance_ns`).
    #[cfg(any(test, feature = "dev-harness"))]
    pub(crate) fn advance(&mut self, interval: f32) -> Result<f32> {
        self.streak_harness_form();
        let units = self.pacing.cadence.units_of_secs(interval);
        self.advance_frame(units)
    }

    /// [`Game::advance`] for the window's measured frame, `ns`
    /// nanoseconds (counted into the tick clock's units exactly).
    pub(crate) fn advance_ns(&mut self, ns: u64) -> Result<f32> {
        // The light streaks' form for this frame, from measured work.
        self.streak_frame_begin(ns as f32 * 1e-9);
        let units = self.pacing.cadence.units_of_ns(ns);
        self.advance_frame(units)
    }

    /// A drawn frame of `units` (`frame_clock::UNITS`).
    fn advance_frame(&mut self, units: i64) -> Result<f32> {
        // With a world loaded it advances by ticks (
        // `nfsu2::frame_clock::TickClock`): each tick's rate is picked from
        // the state before it, inside the catch-up loop: the
        // original's 60 fps (0x57EAE0: 30, 29, 31 of 1/1800 s, once per
        // 1/60 s of real time) or, while a scene holds the cars, its 1/30 s
        // limiter (`frame_clock::SCENE`: 60, 60, 60, 60, 61). At most 3 / 2
        // ticks a drawn frame, the time beyond dropped; drawn frames in
        // between interpolated. No slow-frame branch and no hold
        // after one. The front end without a world ticks the same
        // way at RACE (the original runs 0x57EAE0 in the
        // menus too, measured under Wine): one accumulator and one carry
        // from boot through menu → load → world → menu.
        let rate_now = |g: &Game| if g.scene_frame_cap() { nfsu2::frame_clock::SCENE } else { nfsu2::frame_clock::RACE };
        let mut frame = self.pacing.cadence.begin(units);
        let (mut updates, mut world) = (0usize, 0.0f32);
        while let Some(dt) = self.pacing.cadence.tick(&mut frame, rate_now(self)) {
            // The 60 Hz frame of the tick before, for the streak
            // buffer's commit (`render::streak`), when this frame spans
            // several ticks.
            if updates > 0 {
                self.probe_between_ticks()?;
            }
            updates += 1;
            world += dt;
            self.advance_tick(dt)?;
        }
        self.pacing.frame_updates = updates;
        self.pacing.frame_world = world;
        self.pacing.frame_dropped = frame.dropped as f32 / nfsu2::frame_clock::UNITS as f32;
        let alpha = self.pacing.cadence.alpha(&frame, rate_now(self));
        let drawn = self.smooth_slow_alpha(alpha);
        // The draw clocks run at the ticks' pace under smooth slow
        // motion too (`Game::draw_clocks`).
        self.pacing.slow_tick_alpha = (drawn.to_bits() != alpha.to_bits()).then_some(alpha);
        Ok(drawn)
    }

    /// One tick of [`Game::advance`]: the update, then its snapshot
    /// captured (`render::snapshot`: S_n, the values the tick's
    /// frame draws with; the one before kept as S_{n−1}).
    fn advance_tick(&mut self, dt: f32) -> Result<()> {
        let before = self.replay_at();
        let span = nfsu2::prof::acc::span("tick: snapshot before");
        self.refresh_snapshot();
        drop(span);
        self.step(dt)?;
        let span = nfsu2::prof::acc::span("tick: snapshot capture");
        self.capture_tick(dt);
        drop(span);
        self.pacing.last_update_dt = dt;
        self.smooth_slow_update(before);
        Ok(())
    }

    /// The replay's frame index while one plays.
    pub(crate) fn replay_at(&self) -> Option<usize> {
        match &self.mode {
            Mode::Race(r) => r.replay.playback.as_ref().map(|p| p.at),
            _ => None,
        }
    }

    /// Smooth slow motion: an update that played a recorded frame
    /// keeps the poses and camera before it (S_{n−1}); one that did not
    /// draws from those (so the drawn cars move between the two recorded
    /// frames, not between equal updates).
    pub(crate) fn smooth_slow_update(&mut self, before: Option<usize>) {
        let now = self.replay_at();
        let Mode::Race(r) = &mut self.mode else { return };
        let Some(p) = r.replay.playback.as_mut().filter(|p| p.smooth > 0.0) else { return };
        let prev = &mut self.interp.prev;
        if now != before {
            p.slow_prev = Some((prev.poses.clone(), prev.camera));
        } else if let Some((poses, camera)) = p.slow_prev.clone() {
            prev.poses = poses;
            prev.camera = camera;
        }
    }

    /// Smooth slow motion: the drawn fraction between the recorded
    /// frames (`replay_cams::slow_alpha`); else the cadence's.
    pub(crate) fn smooth_slow_alpha(&self, alpha: f32) -> f32 {
        let Mode::Race(r) = &self.mode else { return alpha };
        let (Some(p), Some(rec)) = (
            r.replay.playback.as_ref().filter(|p| p.smooth > 0.0 && !p.paused && p.slow_prev.is_some()),
            r.replay.recording.as_ref(),
        ) else {
            return alpha;
        };
        let Some(next) = rec.frames.get(p.at) else { return alpha };
        nfsu2::replay_cams::slow_alpha(p.carry, alpha, nfsu2::frame_clock::RACE.update as f32, p.smooth, next.dt)
    }

    /// Draws `alpha` of the way from the tick snapshot before the last
    /// tick to the newest (`render::snapshot`): the frame's own
    /// blended values, computed from the pair; nothing of the simulation
    /// is written for the draw.
    pub(crate) fn draw_between(&mut self, alpha: f32) -> Result<()> {
        self.draw_at(alpha)
    }

    /// The camera mover drawing this frame (`camera_mover`), with each
    /// shot of a scene's own camera (a race's start scene before its cut,
    /// an ending, the career's start, the dyno) as one of its own: the
    /// switch from the chase camera to a scene's, between its shots and
    /// back is a cut, never blended (an ending's first frame was drawn
    /// part-way between the chase camera and the scene's when they stood
    /// under 30 m apart).
    pub(crate) fn view_history_key(&self) -> u32 {
        // Each shot of the scene's camera track its own: a cut between
        // two shots under 30 m apart was blended over the 1/30 s update.
        if let Some(run) = self.scenes.shop.as_ref().filter(|r| r.holds_cars())
            && let Some(track) = run.camera.as_ref()
        {
            return 0x100 + run.scene.camera_shot(track, run.frame.min(run.scene.length)).map_or(0, |s| s as u32 + 1);
        }
        // The fresh chase mover of a blocked crash / jump shot is a mover
        // of its own (its start, the kind 6 → 7 change and the parked
        // camera's return are cuts).
        if let Mode::Race(r) = &self.mode
            && r.replay.playback.is_none()
            && r.camera.parked_chase.is_some()
            && !r.camera.shot.as_ref().is_some_and(|s| s.visible)
        {
            return 0x1_0000 + u32::from(r.camera.events.kind);
        }
        // Free roam's blocked crash / jump shot: its fresh chase mover
        // likewise.
        if matches!(self.mode, Mode::Explore)
            && let Some(c) = self.sim.roam_camera.as_ref()
            && c.parked_chase.is_some()
            && !c.shot.as_ref().is_some_and(|s| s.visible)
        {
            return 0x1_0000 + u32::from(c.events.kind);
        }
        native_mover_type(&self.mode, self.sim.drive.as_ref())
    }

    /// Whether this frame runs under the scenes' frame limiter (1/30 s,
    /// `ShopRun::holds_cars`). Not the loading screen: the game starts a
    /// scene only once its load is done (0x43BAA0), and the clone's loads
    /// draw PC_Loading from inside the load (`LoadingGate`), not here.
    /// An Enter-skipped scene (docs/native-deviations.md "Skipping scenes") has ended, so the cap
    /// ends with it.
    pub(crate) fn scene_frame_cap(&self) -> bool {
        // Agreed deviation (docs/native-deviations.md "High refresh rates"): the shops' and the garage's
        // drive-in / drive-out scenes run at the display's rate; only the
        // cut-scenes (the career's start, race starts, the dyno, the
        // endings: no shop) keep the native 1/30 s.
        self.loads.loading.is_none() && self.scenes.shop.as_ref().is_some_and(|r| r.shop.is_none() && r.holds_cars())
    }
}

/// The frame clock and the cadence's counters (were `Game` fields).
pub(crate) struct Pacing {
    pub(crate) started: Instant,
    /// The developer harness's clock (s) pinned in place of the game
    /// clock (`Game::now` / `seconds`): shots (NFSU2_SHOT_CLOCK), benches
    /// and tests that time their screens themselves. None in the game.
    pub(crate) clock: Option<f64>,
    /// Frames drawn (presentation only; the env-cube faces follow the
    /// tick's schedule index: the original's 0x870818 counts
    /// its loop's frames, one per tick at 60 fps).
    pub(crate) frame_count: u32,
    /// The game's clocks on ticks (`nfsu2::game_clock`): the tick
    /// index, the package clock `Game::now` derives from, the real-time
    /// clock, the sim clock 0x865188 and the world counter 0x7FB724 the
    /// zone lookup's every-8th phase follows (+1 per tick the world runs,
    /// the front end included; reset at world create and teardown; frozen
    /// in the pause). Stepped in `Game::update`, never per drawn
    /// frame.
    pub(crate) clocks: nfsu2::game_clock::GameClock,
    pub(crate) last: Instant,
    /// The world's tick clock: fixed 1/60 s (RACE) or 1/30 s (SCENE)
    /// ticks with the original's tick patterns (0x57EAE0), drawn frames in
    /// between interpolated (`nfsu2::frame_clock::TickClock`; agreed
    /// deviation, docs/native-deviations.md "World update rate").
    pub(crate) cadence: nfsu2::frame_clock::TickClock,
    /// Low Latency (docs/modern-extras.md): the pacer, the VRR probe
    /// (redone every few seconds) and the window's monitor's refresh rate.
    pub(crate) pacer: nfsu2::latency::Pacer,
    pub(crate) vrr: Option<(nfsu2::latency::vrr::Vrr, Instant)>,
    pub(crate) refresh_hz: Option<f64>,
    /// Power Saving (`nfsu2::extras::power_cap`): the window's focus and
    /// visibility (winit's Focused / Occluded), and the battery state with
    /// when it was read (again every few seconds).
    pub(crate) focused: bool,
    pub(crate) occluded: bool,
    pub(crate) battery: Option<(bool, Instant)>,
    pub(crate) last_update_dt: f32,
    /// Motion blur and depth of field (docs/motionblur-dof.md): the world time stepped (s); the drawn
    /// poses' time is this less (1 − α) of the last tick (α is
    /// the frame's, `render::snapshot::FramePoses`).
    pub(crate) world_time: f64,
    /// The tick clock's α of the last frame when smooth slow
    /// motion draws the poses at another (`render::clocks`); else None.
    pub(crate) slow_tick_alpha: Option<f32>,
    /// The world updates the last drawn frame stepped (`advance`).
    pub(crate) frame_updates: usize,
    /// The world time those updates stepped (s).
    pub(crate) frame_world: f32,
    /// The real time the tick cap dropped in that frame (s): the game ran
    /// slower than real time (the performance overlay's speed).
    pub(crate) frame_dropped: f32,
    /// The last frame's time (0x865194, which the camera mover's blend
    /// time scales by, 0x44D190).
    pub(crate) frame_dt: f32,
    /// The settings this tick reads, taken once per tick in
    /// `Game::update` ([`crate::sim::TickSettings`]).
    pub(crate) tick_settings: crate::sim::TickSettings,
}

impl Pacing {
    /// The values `Game::new` starts with (`started` is its start time).
    pub(crate) fn new(started: Instant) -> Pacing {
        Pacing {
            started,
            clock: None,
            frame_count: 0,
            clocks: Default::default(),
            last: started,
            cadence: Default::default(),
            pacer: Default::default(),
            vrr: None,
            refresh_hz: None,
            focused: true,
            occluded: false,
            battery: None,
            last_update_dt: 1.0 / 60.0,
            world_time: 0.0,
            slow_tick_alpha: None,
            frame_updates: 0,
            frame_world: 0.0,
            frame_dropped: 0.0,
            frame_dt: 1.0 / 60.0,
            tick_settings: Default::default(),
        }
    }
}

/// The tick snapshots the frames are drawn between
/// ([`crate::render::snapshot::Snapshots`]; was the state before the last
/// update, swapped into the live objects for the draw).
pub(crate) type Interp = crate::render::snapshot::Snapshots;
// The harness on the tick cadence (dev/ticks.rs `Frames`): the
// harness's entry to `advance`'s tick, so a flow can act between the ticks
// of a drawn frame.
impl Game {
    /// One tick of `advance`'s catch-up loop, `dt` the tick clock's: the
    /// drawn state before it kept for the interpolation, then the step.
    #[cfg(any(test, feature = "dev-harness"))]
    pub(crate) fn harness_tick(&mut self, dt: f32) -> Result<()> {
        self.advance_tick(dt)
    }
}
