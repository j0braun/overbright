//! Tick snapshots (docs/uncapped-fps.md, "Tick snapshots"): the draw as a function
//! of two captured tick states and a blend factor.
//!
//! - [`TickSnapshot`]: what the draw takes from a tick, captured at the end
//!   of each tick ([`Game::capture_tick`], "use, then step": the values
//!   that tick's frame draws with): the drawn cars' poses by car id, the
//!   camera (already shaken), its mover, the scene crowd's frame, the
//!   tick's dt and the cut flag. Kept in a two-slot ring ([`Snapshots`]:
//!   S_{n−1}, S_n); the slots' buffers are reused, so a capture allocates
//!   nothing in the steady state.
//! - [`FramePoses`]: one drawn frame's values, `lerp(S_{n−1}, S_n, α)`
//!   computed into the frame's own copies ([`Game::frame_poses`]). The
//!   draw reads them through `FrameView`/`SceneGame`/`SceneCtx` and never
//!   writes the simulation's cars, camera, crowd or particles: the old
//!   swap-in / swap-out of blended poses (`draw_between`) and the water's
//!   own `lag` are gone.
//!
//! - The drawn objects that step on the tick besides the cars and
//!   the camera: the particles by id (`nfsu2::particles::between`, the
//!   carried ones with their owner car's drawn pose), the rain's falling
//!   drops and lens drops (`nfsu2::weather::RainSnapshot`), the moved
//!   placements (world animations and knocked-over props, `Drive::posed`,
//!   `nfsu2::interpolate::transform_at`), photo mode's rig and the dyno's
//!   rollers.
//!
//! - The front end's garage scene (its turntable, camera, FE car place,
//!   hydraulics, part animations and particles): [`TickSnapshot::garage`],
//!   blended by `Game::frame_garage` (`crate::garage::snapshot`).
//!
//! Not in the snapshot yet (each listed in the doc as a follow-up): the
//! HUD's and the menus' layout (pure reads of the tick state,
//! drawn as S_n). The garage's own scene (its car, turntable, part
//! animations and particles) is blended from the garage snapshot
//! (`garage::snapshot`). The native
//! visuals step on the tick (`TickSnapshot::vis`, `render::visuals`); the
//! streak buffer commits per tick from each tick's own frame
//! (`render::streak`: a probe draw where no drawn frame stands on a tick).
//! The shaders' clocks (`render::clocks`, [`TickSnapshot::clocks`], [`FramePoses::clocks`]).
use crate::*;
use nfsu2::interpolate::{camera_at, pose_at, CarPose};

/// What the draw takes from one tick (S_n).
#[derive(Clone, Debug, Default)]
pub(crate) struct TickSnapshot {
    /// The ring's tick count (1 for the first capture).
    pub(crate) tick: u64,
    /// The drawn cars in [`Game::drawn_car_states`]'s order: their ids
    /// (`DrawId::key`) and poses.
    pub(crate) keys: Vec<u64>,
    pub(crate) poses: Vec<CarPose>,
    /// The drive's camera as the tick left it (shaken).
    pub(crate) camera: Option<Camera>,
    /// The camera mover (`Game::view_history_key`).
    pub(crate) mover: u32,
    /// The scene crowd's frame.
    pub(crate) crowd: Option<f32>,
    /// The tick's dt (s).
    pub(crate) dt: f32,
    /// A cut between the previous tick and this one: the camera's mover
    /// changed (the crash / jump camera, a scene's shot, the chase camera
    /// back). The frames between the two draw the camera and the crowd at
    /// S_n (α = 1 semantics), as the original cuts.
    pub(crate) cut: bool,
    /// The draw's clocks as the tick left them.
    pub(crate) clocks: super::clocks::TickClocks,
    /// The drive the fields below are of (its car's id key; 0
    /// without a drive): a frame blends them only within one drive (a
    /// world load starts the particle ids and the placements anew).
    pub(crate) drive_key: u64,
    /// The drive's particles (in their system's order, ids ascending).
    pub(crate) particles: Vec<nfsu2::particles::Particle>,
    /// The particle owners' transforms, by owner index
    /// ([`Game::fill_particle_owners`]).
    pub(crate) owners: Vec<Option<glam::Mat4>>,
    /// The rain's drops and lens drops.
    pub(crate) rain: nfsu2::weather::RainSnapshot,
    /// The moved placements (`Drive::posed`), by placement.
    pub(crate) posed: Vec<(usize, glam::Mat4)>,
    /// Photo mode's rig.
    pub(crate) photo: Option<nfsu2::photo::Rig>,
    /// The dyno's rollers' angles.
    pub(crate) rollers: Option<[f32; 4]>,
    /// The drive's native visuals (`render::visuals`; None: no
    /// drive) and the overbright level the tick's frame draws with.
    pub(crate) vis: Option<crate::render::visuals::DriveVis>,
    pub(crate) overbright: nfsu2::render::postfx::OverbrightLevel,
    /// The front end's garage scene (`crate::garage::snapshot`).
    pub(crate) garage: crate::garage::snapshot::GarageSnapshot,
    /// The settings the tick stepped with (`sim::TickSettings`): the
    /// draw shows the tick-stepped state with these, not with a value
    /// changed since (docs/frame-settings.md).
    pub(crate) settings: crate::sim::TickSettings,
}

/// The two-slot ring of tick snapshots (S_{n−1}, S_n).
#[derive(Default)]
pub(crate) struct Snapshots {
    pub(crate) prev: TickSnapshot,
    pub(crate) cur: TickSnapshot,
    /// The environment cube's eye of the last six ticks, by
    /// schedule index mod 6 ([`EnvEye`]; a frame renders each face from
    /// the newest of them at which it was due).
    pub(crate) env: [Option<EnvEye>; 6],
    /// The last few ticks as the tick-stepped enhancement effects
    /// read them (`render::enh::tick_views`), oldest first.
    pub(crate) recent: std::collections::VecDeque<nfsu2::render::enh::tick_views::TickRecord>,
    /// The motion blur's state of the last few ticks
    /// (`render::enh::blur_ticks`), oldest first.
    pub(crate) blur: std::collections::VecDeque<crate::render::enh::blur_ticks::BlurTick>,
}

/// docs/uncapped-fps.md, "Environment cube on ticks": where tick
/// m's cube faces are rendered from (0x61AB60: the view's car + 0.95 m up,
/// oriented by the main camera, view 1), taken from S_m, and the scene
/// clock of that tick (the faces' animated textures).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct EnvEye {
    /// The tick's face-schedule index (0x870818 of the original's frame:
    /// the tick count before the tick, [`env_index`]).
    pub(crate) index: u64,
    pub(crate) centre: Vec3,
    /// The camera's right, down and forward (`CameraAxes::look`).
    pub(crate) axes: (Vec3, Vec3, Vec3),
    pub(crate) time: f32,
}

impl EnvEye {
    /// The eye of a car `state` under `camera` at `time`.
    pub(crate) fn of(index: u64, state: &CarState, camera: &Camera, time: f32) -> EnvEye {
        let axes = CameraAxes::look(camera.forward(), camera.up);
        EnvEye { index, centre: state.position + Vec3::Z * 0.95, axes: (axes.right, axes.down, axes.forward), time }
    }
}

/// The face-schedule index of the state after tick count `tick` (the
/// original's counter 0x870818 counts its frames from 0 and is read
/// before it steps, so the frame of the first tick reads 0).
pub(crate) fn env_index(tick: u64) -> u64 {
    tick.saturating_sub(1)
}

/// The environment cube a drawn frame stands at: the schedule
/// index of the tick it draws (`drawn`: the newest tick at α = 1, else
/// the one before, the S_{n−1} the frame starts from) and the eyes of the
/// ticks before it.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct EnvFrame {
    pub(crate) drawn: u64,
    /// The frame draws the newest tick as the game holds it (α = 1): its
    /// own car and camera are that tick's eye (and any write after the
    /// tick, as the frame draws it).
    pub(crate) live: bool,
    pub(crate) eyes: [Option<EnvEye>; 6],
}

impl EnvFrame {
    /// The eye of schedule index `m`, when it was captured.
    pub(crate) fn eye(&self, m: u64) -> Option<&EnvEye> {
        self.eyes[(m % 6) as usize].as_ref().filter(|e| e.index == m)
    }
}

/// One drawn frame's own values, between the ring's two snapshots.
pub(crate) struct FramePoses {
    /// Where the frame stands between the two ticks (1: on the newest).
    pub(crate) alpha: f32,
    /// The newest tick's dt (s).
    pub(crate) dt: f32,
    /// The drive's car as drawn (a copy; the drive's own is never written).
    player: Option<CarState>,
    player_key: u64,
    /// The other cars drawn between the ticks, by id (absent: drawn as the
    /// tick left them).
    cars: Vec<(u64, CarState)>,
    /// The camera drawn between the ticks (None: as the tick left it).
    pub(crate) camera: Option<Camera>,
    /// The scene crowd's frame between the ticks.
    crowd: Option<f32>,
    /// The frame's clocks between the ticks (the draw's only
    /// time: `render::clocks`).
    pub(crate) clocks: super::clocks::DrawClocks,
    /// The environment cube's tick and eyes.
    pub(crate) env: EnvFrame,
    /// The particles as drawn (None: the drive's own, as the
    /// tick left them) and their size factors (empty: all 1).
    particles: Option<Vec<nfsu2::particles::Particle>>,
    particle_scale: Vec<f32>,
    /// The rain as drawn (None: the drive's own).
    rain: Option<nfsu2::weather::RainSnapshot>,
    /// The moved placements as drawn (None: the drive's own `posed`).
    posed: Option<std::collections::HashMap<usize, glam::Mat4>>,
    /// Photo mode's rig as drawn (None: its own).
    pub(crate) photo_rig: Option<nfsu2::photo::Rig>,
    /// The dyno's rollers' angles as drawn (None: their own).
    pub(crate) rollers: Option<[f32; 4]>,
    /// The native visuals as drawn (`Game::frame_vis`).
    pub(crate) vis: crate::render::visuals::FrameVis,
    /// The newest tick (`GameClock::tick`, the streak buffer's
    /// commit slot, `render::streak`) and the ticks the drawn frame took.
    pub(crate) tick: u64,
    pub(crate) frame_ticks: u32,
    /// The garage scene as drawn (None: as the tick left it;
    /// `Game::frame_garage`).
    pub(crate) garage: Option<crate::garage::snapshot::GarageDrawn>,
    /// The settings the newest tick stepped with (None: no tick captured).
    pub(crate) stepped: Option<crate::sim::TickSettings>,
    /// The motion blur's motion from the ticks (None: from the last drawn
    /// frame; `Game::frame_blur`).
    pub(crate) blur: Option<nfsu2::render::blur_dof::TickMotion>,
}

impl FramePoses {
    /// A frame on the newest tick (α = 1): every value as the tick left it.
    pub(crate) fn live(drive: Option<&Drive>) -> FramePoses {
        FramePoses {
            alpha: 1.0,
            dt: 1.0 / 60.0,
            player: drive.map(|d| {
                // A scene car on its own 30 fps schedule as drawn at the
                // world's time (`CarPose::drawn`).
                let mut s = d.state.clone();
                if s.nis.is_some_and(|m| m.pace.is_some()) {
                    CarPose::drawn(&d.state).apply(&mut s);
                }
                s
            }),
            player_key: drive.map_or(0, |d| d.car_id.key()),
            cars: Vec::new(),
            camera: None,
            crowd: None,
            clocks: super::clocks::DrawClocks::still(0.0, 0.0, drive.map(|d| d.clock)),
            env: EnvFrame::default(),
            particles: None,
            particle_scale: Vec::new(),
            rain: None,
            posed: None,
            photo_rig: None,
            rollers: None,
            vis: crate::render::visuals::FrameVis::live(drive.map(|d| &d.vis.out), Default::default()),
            tick: 0,
            frame_ticks: 1,
            garage: None,
            stepped: None,
            blur: None,
        }
    }

    /// The drive's car as drawn this frame (there whenever a drive is).
    pub(crate) fn player(&self) -> &CarState {
        self.player.as_ref().expect("a frame's poses are built with its drive")
    }

    /// The car `id` (whose tick state is `live`) as drawn this frame.
    pub(crate) fn car<'a>(&'a self, id: &DrawId, live: &'a CarState) -> &'a CarState {
        let key = id.key();
        if key == self.player_key
            && let Some(p) = self.player.as_ref()
        {
            return p;
        }
        self.cars.iter().find(|(k, _)| *k == key).map_or(live, |(_, s)| s)
    }

    /// The scene crowd's frame as drawn.
    pub(crate) fn crowd_frame(&self, c: &SceneCrowd) -> f32 {
        self.crowd.unwrap_or(c.frame)
    }

    /// The frame's camera: `live` (the drive's) when something wrote it
    /// after the tick (`late`: photo mode's camera for the frame), else the blend between the ticks, else the tick's.
    pub(crate) fn camera_for(&self, live: &Camera, late: bool) -> Camera {
        if late { *live } else { self.camera.unwrap_or(*live) }
    }

    /// The particles as drawn: `live` itself (α = 1), or the frame's own
    /// list between the ticks.
    pub(crate) fn particles<'a>(&'a self, live: &'a [nfsu2::particles::Particle]) -> &'a [nfsu2::particles::Particle] {
        self.particles.as_deref().unwrap_or(live)
    }

    /// The drawn particles' size factors (empty: all 1; a particle born in
    /// the newest tick grows in between the ticks).
    pub(crate) fn particle_scale(&self) -> &[f32] {
        &self.particle_scale
    }

    /// The rain as drawn between the ticks (None: the drive's own).
    pub(crate) fn rain(&self) -> Option<&nfsu2::weather::RainSnapshot> {
        self.rain.as_ref()
    }

    /// The moved placements as drawn: `live` (the drive's), or the
    /// frame's own blend between the ticks.
    pub(crate) fn posed<'a>(
        &'a self,
        live: &'a std::collections::HashMap<usize, glam::Mat4>,
    ) -> &'a std::collections::HashMap<usize, glam::Mat4> {
        self.posed.as_ref().unwrap_or(live)
    }
}

/// Every field of two cameras bit for bit (`Camera` has no `PartialEq`).
fn same_camera(a: &Camera, b: &Camera) -> bool {
    let bits = |c: &Camera| {
        (
            [c.position.x, c.position.y, c.position.z, c.yaw, c.pitch, c.fov_y, c.up.x, c.up.y, c.up.z, c.near, c.far]
                .map(f32::to_bits),
            c.game_fov.map(f32::to_bits),
            [c.jitter.x.to_bits(), c.jitter.y.to_bits()],
        )
    };
    bits(a) == bits(b)
}

impl Game {
    /// The cars the world view draws, in a fixed order (the interpolation
    /// pairs them by it), with their ids: the player, the race's rivals
    /// and traffic, free roam's traffic and encounter cars, a shop scene's
    /// companion.
    pub(crate) fn drawn_car_states<'a>(&'a self, mut each: impl FnMut(u64, &'a CarState)) {
        if let Some(d) = self.sim.drive.as_ref() {
            each(d.car_id.key(), &d.state);
        }
        if let Mode::Race(r) = &self.mode {
            for x in &r.racers.rivals {
                each(x.id.key(), &x.state);
            }
            if let Some(t) = r.racers.traffic.as_ref() {
                for c in &t.cars {
                    each(c.id.key(), &c.state);
                }
            }
        }
        if let Some(t) = self.sim.traffic.as_ref() {
            for c in &t.cars {
                each(c.id.key(), &c.state);
            }
        }
        if let Some(e) = self.sim.encounters.as_ref() {
            for c in &e.cars {
                each(c.id.key(), &c.state);
            }
        }
        if let Some(c) = self.scenes.shop.as_ref().and_then(|run| run.companion.as_ref()) {
            each(c.id.key(), &c.state);
        }
    }

    /// The end of a tick: S_n becomes S_{n−1} and the tick's state
    /// is captured into S_n (a pure read of the game; the slot's buffers
    /// reused).
    pub(crate) fn capture_tick(&mut self, dt: f32) {
        let mut ring = std::mem::take(&mut self.interp);
        std::mem::swap(&mut ring.prev, &mut ring.cur);
        let s = &mut ring.cur;
        s.tick = ring.prev.tick + 1;
        s.keys.clear();
        s.poses.clear();
        self.drawn_car_states(|k, c| {
            s.keys.push(k);
            s.poses.push(CarPose::drawn(c));
        });
        s.camera = self.sim.drive.as_ref().map(|d| d.camera);
        s.mover = self.view_history_key();
        s.crowd = self.scenes.crowd.as_ref().map(|c| c.frame);
        s.dt = dt;
        s.cut = s.mover != ring.prev.mover;
        s.clocks = self.tick_clocks();
        s.settings = self.pacing.tick_settings;
        // The tick's environment-cube eye (the drive's car and
        // camera as the tick left them, the tick's scene clock).
        let index = env_index(self.pacing.clocks.tick);
        ring.env[(index % 6) as usize] =
            self.sim.drive.as_ref().map(|d| EnvEye::of(index, &d.state, &d.camera, self.seconds()));
        self.capture_drawn_objects(s);
        s.vis = self.sim.drive.as_ref().map(|d| d.vis.out);
        s.overbright = self.visuals.drawn;
        self.capture_garage(&mut s.garage);
        self.interp = ring;
        self.record_tick_view();
        self.record_blur_tick(self.pacing.clocks.tick, self.interp.cur.clocks.world_time);
    }

    /// The drawn objects a tick leaves into `s` (a pure read; the
    /// buffers reused): the particles and their owners, the rain, the
    /// moved placements, photo mode's rig, the dyno's rollers. Called by
    /// the capture and, unconditionally, by `refresh_snapshot` (a change
    /// outside the ticks: S_n is then where the next tick starts from).
    fn capture_drawn_objects(&self, s: &mut TickSnapshot) {
        let d = self.sim.drive.as_ref();
        s.drive_key = d.map_or(0, |d| d.car_id.key());
        s.particles.clear();
        s.posed.clear();
        if let Some(d) = d {
            s.particles.extend_from_slice(&d.effects.system.particles);
            d.weather.capture(&mut s.rain);
            s.posed.extend(d.posed.iter().map(|(&p, &t)| (p, t)));
            s.posed.sort_unstable_by_key(|e| e.0);
        }
        let mut owners = std::mem::take(&mut s.owners);
        self.fill_particle_owners(|_, c| c, &mut owners);
        s.owners = owners;
        s.photo = self.photo.as_ref().map(|p| p.rig);
        s.rollers = self.dyno.rollers.as_ref().map(|r| r.angles);
    }

    /// Before a tick, S_n made the state the tick starts from
    /// when something outside the ticks changed what the draw reads since
    /// the last capture (the harness's set-up steps, a load, a menu action
    /// that moved a car or the camera): the frames after the tick then
    /// blend from where the world stood, as before (the old interpolation
    /// took its "before" at each tick's start). A pure read and no change
    /// when nothing moved.
    pub(crate) fn refresh_snapshot(&mut self) {
        // The clocks as the tick starts from them (a load's world
        // clock, a harness's clock set between the ticks).
        self.interp.cur.clocks = self.tick_clocks();
        // The visuals as a step outside the ticks left them.
        self.interp.cur.vis = self.sim.drive.as_ref().map(|d| d.vis.out);
        self.interp.cur.overbright = self.visuals.drawn;
        let cur = &self.interp.cur;
        let mut same = true;
        let mut i = 0;
        self.drawn_car_states(|k, c| {
            same &= cur.keys.get(i) == Some(&k) && cur.poses.get(i) == Some(&CarPose::drawn(c));
            i += 1;
        });
        same &= i == cur.keys.len()
            && match (cur.camera.as_ref(), self.sim.drive.as_ref()) {
                (Some(a), Some(d)) => same_camera(a, &d.camera),
                (None, None) => true,
                _ => false,
            }
            && cur.crowd.map(f32::to_bits) == self.scenes.crowd.as_ref().map(|c| c.frame.to_bits())
            && cur.mover == self.view_history_key();
        // The drawn objects (particles, rain, placements, photo
        // rig, rollers) always: a copy as cheap as comparing them.
        let mut ring = std::mem::take(&mut self.interp);
        self.capture_drawn_objects(&mut ring.cur);
        self.capture_garage(&mut ring.cur.garage);
        self.interp = ring;
        if same {
            return;
        }
        let mut ring = std::mem::take(&mut self.interp);
        let s = &mut ring.cur;
        s.keys.clear();
        s.poses.clear();
        self.drawn_car_states(|k, c| {
            s.keys.push(k);
            s.poses.push(CarPose::drawn(c));
        });
        s.camera = self.sim.drive.as_ref().map(|d| d.camera);
        s.mover = self.view_history_key();
        s.crowd = self.scenes.crowd.as_ref().map(|c| c.frame);
        self.interp = ring;
    }

    /// The particle owners' transforms, by owner index (the cars
    /// `particle_effects` hands `Effects::step`: the player, then a race's
    /// rivals or free roam's encounter slots, parked ones as holes), each
    /// car's state as `at` gives it.
    pub(crate) fn particle_owner_transforms_at<'a>(
        &'a self,
        at: impl Fn(&DrawId, &'a CarState) -> &'a CarState,
    ) -> Vec<Option<glam::Mat4>> {
        let mut v = Vec::new();
        self.fill_particle_owners(at, &mut v);
        v
    }

    /// [`Game::particle_owner_transforms_at`] into `v` (cleared first;
    /// its buffer reused).
    pub(crate) fn fill_particle_owners<'a>(
        &'a self,
        at: impl Fn(&DrawId, &'a CarState) -> &'a CarState,
        v: &mut Vec<Option<glam::Mat4>>,
    ) {
        v.clear();
        let Some(d) = self.sim.drive.as_ref() else { return };
        let of = |s: &CarState| glam::Mat4::from_rotation_translation(s.orientation, s.position);
        v.push(Some(of(at(&d.car_id, &d.state))));
        match &self.mode {
            Mode::Race(r) => v.extend(r.racers.rivals.iter().map(|r| Some(of(at(&r.id, &r.state))))),
            Mode::Explore => v.extend(
                self.sim.encounters.iter().flat_map(|e| e.cars.iter()).map(|c| c.active.then(|| of(at(&c.id, &c.state)))),
            ),
            _ => {}
        }
    }

    /// The frame's own values `alpha` of the way from S_{n−1} to
    /// S_n (a pure read). The cut rules, in one place (each as before):
    /// - α ≥ 1, no drive, or no S_{n−1} yet (the first tick): the newest
    ///   tick's values;
    /// - the cars: blended only when the drawn set has as many cars as
    ///   S_{n−1}; a car is drawn as S_n where it jumped over 30 m (a
    ///   teleport, a reset to the road, a respawn: `pose_at`), where
    ///   another car stands at its index in S_{n−1}, or where something
    ///   wrote it after the tick (the harness's frozen car);
    /// - the camera and the crowd: drawn at S_n on a mover change
    ///   ([`TickSnapshot::cut`]: crash / jump camera, scene shots, start
    ///   and end of a scene), a camera jump over 30 m or a change of camera
    ///   kind (`camera_at`), a crowd frame stepping back or over 4 frames
    ///   (a new scene); the camera also where something wrote it after the
    ///   tick (photo mode's: [`FramePoses::camera_for`]).
    /// A change outside the ticks is not a cut: `refresh_snapshot` makes
    /// it S_n before the next tick, which then blends from it.
    pub(crate) fn frame_poses(&self, alpha: f32) -> FramePoses {
        let drive = self.sim.drive.as_ref();
        let mut f = FramePoses::live(drive);
        f.clocks = self.draw_clocks(alpha);
        f.vis = self.frame_vis(alpha);
        f.tick = self.pacing.clocks.tick;
        f.frame_ticks = u32::try_from(self.pacing.frame_updates).unwrap_or(u32::MAX);
        f.garage = self.frame_garage(alpha);
        let (prev, cur) = (&self.interp.prev, &self.interp.cur);
        f.stepped = (cur.tick != 0).then_some(cur.settings);
        // The cube stands at the tick the frame draws (S_n at
        // α = 1, else S_{n−1}).
        let newest = env_index(self.pacing.clocks.tick);
        f.env = EnvFrame { drawn: newest, live: true, eyes: self.interp.env };
        let between = drive.is_some() && alpha < 1.0 && !prev.poses.is_empty();
        f.blur = self.frame_blur(f.tick, alpha, !between);
        let Some(d) = drive.filter(|_| alpha < 1.0 && !prev.poses.is_empty()) else { return f };
        f.alpha = alpha;
        f.env.drawn = newest.saturating_sub(1);
        f.env.live = false;
        f.dt = cur.dt;
        let mut live: Vec<(u64, &CarState)> = Vec::with_capacity(prev.poses.len());
        self.drawn_car_states(|k, s| live.push((k, s)));
        if live.len() == prev.poses.len() {
            for (i, &(key, s)) in live.iter().enumerate() {
                let now = CarPose::drawn(s);
                let fresh = cur.keys.get(i) == Some(&key) && cur.poses.get(i) == Some(&now);
                if !fresh || prev.keys.get(i) != Some(&key) {
                    continue;
                }
                if let Some(b) = pose_at(&prev.poses[i], &now, alpha, f.dt) {
                    let mut drawn = s.clone();
                    b.apply(&mut drawn);
                    if key == f.player_key {
                        f.player = Some(drawn);
                    } else {
                        f.cars.push((key, drawn));
                    }
                }
            }
        }
        // The drawn objects of the drive between its two ticks.
        if prev.drive_key == d.car_id.key() {
            self.frame_objects(d, &mut f, alpha);
        }
        let cut = cur.cut;
        let fresh = cur.camera.is_some_and(|c| same_camera(&c, &d.camera));
        if !cut && fresh {
            f.camera = prev.camera.and_then(|a| camera_at(&a, &d.camera, alpha));
        }
        // A scene's characters (stepped with the world, 30 frames a
        // second) at the same point between the ticks.
        if let (Some(c), Some(a), Some(n)) = (self.scenes.crowd.as_ref(), prev.crowd, cur.crowd)
            && !cut
            && n.to_bits() == c.frame.to_bits()
            && n >= a
            && n - a < 4.0
        {
            f.crowd = Some(a + (n - a) * alpha);
        }
        f
    }

    /// The frame's particles, rain, moved placements, photo rig
    /// and rollers `alpha` (0 ≤ α < 1) of the way from S_{n−1} to the
    /// tick's live state (S_n; `refresh_snapshot` makes a change outside
    /// the ticks S_n before the next tick), each exactly S_{n−1} at α = 0.
    fn frame_objects(&self, d: &Drive, f: &mut FramePoses, alpha: f32) {
        let prev = &self.interp.prev;
        // The particles by id. The carried ones (the nitrous flame and the
        // purge, keyed to their owner car, `FLAME_KEY` + index) are drawn
        // with their owner's drawn pose: blended in the owner's frame
        // (S_{n−1}'s place in S_{n−1}'s owner, S_n's in S_n's), then put
        // in the drawn owner; a newborn at S_n's place relative to the car
        // (drawn × newest⁻¹). Drawn with the car's blended pose at their
        // world place they would sit up to one tick ahead of it, inside
        // the body at speed (the flames went missing above 60 Hz). The
        // original draws them in the frame that emits them, so relative
        // to the car they sit where it put them.
        if !d.effects.order.unblended_particles {
            let owners_now = self.particle_owner_transforms_at(|_, s| s);
            let owners_drawn = self.particle_owner_transforms_at(|id, s| f.car(id, s));
            let carry = |older: Option<&nfsu2::particles::Particle>, p: &nfsu2::particles::Particle| {
                let owner = p.key.checked_sub(FLAME_KEY)? as usize;
                let (Some(Some(now)), Some(Some(drawn))) = (owners_now.get(owner), owners_drawn.get(owner)) else {
                    return None;
                };
                Some(match (older, prev.owners.get(owner)) {
                    (Some(o), Some(Some(before))) => {
                        let local = now.inverse().transform_point3(p.position);
                        drawn.transform_point3(before.inverse().transform_point3(o.position).lerp(local, alpha))
                    }
                    _ => (*drawn * now.inverse()).transform_point3(p.position),
                })
            };
            let (mut out, mut scale) = (Vec::new(), Vec::new());
            nfsu2::particles::between(&prev.particles, &d.effects.system.particles, alpha, carry, &mut out, &mut scale);
            f.particles = Some(out);
            f.particle_scale = scale;
        }
        // The rain.
        let mut now = nfsu2::weather::RainSnapshot::default();
        d.weather.capture(&mut now);
        let mut rain = nfsu2::weather::RainSnapshot::default();
        nfsu2::weather::RainSnapshot::between(&prev.rain, &now, alpha, &mut rain);
        f.rain = Some(rain);
        // The moved placements (world animations, knocked-over props): a
        // placement S_{n−1} had not moved yet stood at its rest pose.
        if d.posed.iter().any(|(p, t)| prev.posed.binary_search_by_key(p, |e| e.0).map_or(true, |i| prev.posed[i].1 != *t)) {
            let rest = |p: usize| d.world.placements.get(p).map(|x| x.transform);
            f.posed = Some(
                d.posed
                    .iter()
                    .map(|(&p, t)| {
                        let before = prev.posed.binary_search_by_key(&p, |e| e.0).ok().map(|i| prev.posed[i].1).or_else(|| rest(p));
                        (p, before.and_then(|a| nfsu2::interpolate::transform_at(&a, t, alpha)).unwrap_or(*t))
                    })
                    .collect(),
            );
        }
        // Photo mode's rig (moved on the ticks from the held keys):
        // blended unless something set it after the tick (a key).
        if let (Some(a), Some(b), Some(live)) = (prev.photo, self.interp.cur.photo, self.photo.as_ref().map(|p| p.rig))
            && b == live
        {
            f.photo_rig = Some(nfsu2::photo::Rig::between(&a, &b, alpha));
        }
        // The dyno's rollers, each the way its wheel turned it.
        if let (Some(a), Some(r)) = (prev.rollers, self.dyno.rollers.as_ref()) {
            f.rollers = Some(r.angles_at(&a, alpha, f.dt));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pose(x: f32) -> CarPose {
        CarPose::of(&CarState::new(Vec3::new(x, 0.0, 0.0), 0.0))
    }

    fn ring(prev: &[(u64, f32)], cur: &[(u64, f32)]) -> Snapshots {
        let snap = |cars: &[(u64, f32)]| TickSnapshot {
            keys: cars.iter().map(|c| c.0).collect(),
            poses: cars.iter().map(|c| pose(c.1)).collect(),
            dt: 1.0 / 60.0,
            ..Default::default()
        };
        Snapshots { prev: snap(prev), cur: snap(cur), env: Default::default(), recent: Default::default(), blur: Default::default() }
    }

    /// The frame's camera: a late write (photo mode's) wins;
    /// else the blend; else the tick's.
    #[test]
    fn late_camera_writes_win() {
        let mut f = FramePoses::live(None);
        let live = Camera::new(Vec3::ONE, 0.2, 0.0);
        assert!(same_camera(&f.camera_for(&live, false), &live));
        f.camera = Some(Camera::new(Vec3::ZERO, 0.1, 0.0));
        assert!(same_camera(&f.camera_for(&live, false), &f.camera.unwrap()));
        assert!(same_camera(&f.camera_for(&live, true), &live));
    }

    /// The ring's slots swap at each capture: S_n becomes S_{n−1}.
    #[test]
    fn ring_keeps_the_last_two_ticks() {
        let r = ring(&[(1, 0.0)], &[(1, 1.0)]);
        assert_eq!(r.prev.poses[0].position.x, 0.0);
        assert_eq!(r.cur.poses[0].position.x, 1.0);
    }

    /// The frame's own particles, when there are any; else the live ones.
    #[test]
    fn drawn_particles_are_the_frames_own() {
        let mut f = FramePoses::live(None);
        let live = vec![nfsu2::particles::Particle {
            description: 0,
            palette: 0,
            key: 0,
            life: 0,
            angle: 0,
            speed: 0,
            carried: 0,
            position: Vec3::ZERO,
            direction: [0; 3],
            frame: 0,
            id: 0,
        }];
        assert!(std::ptr::eq(f.particles(&live), live.as_slice()));
        let mut moved = live.clone();
        moved[0].position = Vec3::X;
        f.particles = Some(moved);
        assert_eq!(f.particles(&live)[0].position, Vec3::X);
        assert_eq!(live[0].position, Vec3::ZERO);
    }
}
