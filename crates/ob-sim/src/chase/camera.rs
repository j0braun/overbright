use glam::{Mat3, Vec3};

use super::*;
use crate::car::{CameraView, Car};
use crate::drive::CarState;
use crate::draw::camera::Camera;

/// The game's in-race camera for one car.
#[derive(Clone, Debug)]
pub struct ChaseCamera {
    view: CameraView,
    tuning: Tuning,
    eye: Ease3,
    look: Ease3,
    lean: Ease3,
    pub(super) heading: Ease3,
    fov: Ease,
    /// The last four frames of acceleration (0x446E70 with 4 slots).
    accel: [Vec3; 4],
    frames: usize,
    last_velocity: Option<Vec3>,
    /// Seconds since the camera was created: the shake's noise clock while
    /// the caller has not handed the sim clock ([`ChaseCamera::set_clock`]).
    age: f32,
    /// The sim clock 0x865188 in seconds (× 0.00025), the shake's noise
    /// clock (0x452ED0 → 0x441340), when the caller hands it.
    clock: Option<f32>,
    /// The speed the shake graph is scaled to, m/s (0x452ED0: S =
    /// [car +0x10] +0x20, 100 when car +0x10 is null; see `new`).
    trap_speed: f32,
    /// A nitrous shot burns this frame (0x59FA40 non-zero on car +0x40 and
    /// 0x5ECDC0 zero: the flame, not the purge).
    nitrous_flame: bool,
    /// Seconds since the mover's time stamp (+0x58): the ctor (0x4516D5)
    /// and, every update with the car's world loaded (+0x5CC) and more
    /// than one wheel down (car +0x424 > 1), 0x44DE90 at 0x44DF41 — after
    /// 0x445C20 (0x44DEA9) has used it. The chase direction's vertical part
    /// fades out over two seconds of it (0x445C20), so only a car in the
    /// air for 2 s loses it. **Verified** (disassembly) and **observed**
    /// under Wine (2026-11-04: +0x58 equal to the sim clock 0x865188 frame
    /// after frame in free roam; view 1's forward z following the car's
    /// pitch for minutes in free roam and the stage-1 circuit). The clone
    /// faded it over the camera's first two seconds of life, so its chase
    /// frame stayed level for good (docs/notes/parity/world-systems-parity.md).
    since_ground: f32,
    fresh: bool,
    /// Looking back (mover +0x51, set through vtable +0x14 = 0x445A30 from
    /// the player's look-back key, code 0x7E in 0x602BE0, held).
    look_back: bool,
    /// Split screen (screen layout 0x8707DC = 3): picks the second
    /// look-back entry.
    split: bool,
    /// The race's mode (0x453570 reads the drag, drift and URL flags).
    race: crate::race::RaceMode,
    /// The impact shake's offset for this update (0x572870, world m; zero
    /// when none runs, on its strobe's odd steps or while the world's time
    /// scale is below 1), applied by 0x572A60 at 0x453B55 right after the
    /// look-at, before the speed shake ([`ChaseCamera::set_impact`]).
    impact: Vec3,
}

/// The look-back eye (0x445EB0: table 0x7EC400, 16 bytes a row, row 1 when
/// the screen layout 0x8707DC is 3) in the view's car space, and its tilt
/// (0x787080, binary angle). Used only while the player's +0x2B is clear
/// (0x5EB530); see [`ChaseCamera::set_look_back`].
/// Whether the player's +0x2B is clear (0x5EB530), which picks the table
/// over the mirror. **Verified**: the player's constructor 0x5F1680
/// zeroes the block +0x20..+0x4F and sets +0x2B = 1 through the block's
/// base (0x5F17A5: `mov byte [edx+0xb], 1` with edx = player + 0x20, which
/// is why a search for byte stores at +0x2B found none); nothing else
/// writes it (no other store to the player's +0x2B; +0x20 / +0x21 are
/// refilled from the player-settings bytes 0x83AA79 / 0x83AA7A, +0x2B is not).
/// It read 1 under Wine in free roam and races, so false.
pub(super) const LOOK_BACK_TABLE: bool = false;
pub(super) const LOOK_BACK: [(Vec3, u16); 2] =
    [(Vec3::new(-17.2, 0.0, 6.0), 0x1450), (Vec3::new(-15.8, 0.0, 4.4), 0x0DD4)];
/// The wall radius of the world push (vtable +0x30, 0x445A60): 0.7
/// (0x787090), or 3 (0x787094) while looking back.
pub const WALL_RADIUS: [f32; 2] = [0.7, 3.0];

impl ChaseCamera {
    /// A camera of the given kind (see [`CameraView::kind`]), using the car's
    /// own entry for it.
    pub fn new(car: &Car, kind: u16) -> ChaseCamera {
        let views = &car.car_type.cameras;
        // 0x5ECCB0: an unknown kind falls back to kind 0.
        let view = views
            .iter()
            .find(|v| v.kind == kind)
            .or_else(|| views.iter().find(|v| v.kind == 0))
            .copied()
            .unwrap_or(CameraView { kind: 1, tilt: 0, eye: Vec3::new(-4.7, 0.0, 1.7) });
        let tuning = Tuning::blended(view.kind);
        // 0x452ED0: S = [car +0x10] +0x20 (100 when car +0x10 is null):
        // the racer info's +0x30, the performance object's +0x10, the
        // gearing-limited top speed (**verified** writer
        // 0x5B5CB5 in 0x5B5AC0, **measured**: 73.76 for the 206, 67.05
        // 240SX, 75.10 MIATA, as read under Wine). Every car is built
        // from a racer info (0x604270), so the fallback is only for a
        // handling never set up.
        let g = car.car_type.handling.gearing_top_speed;
        let trap_speed = if g > 0.0 { g } else { SHAKE_TOP_SPEED };
        let mut eye = Ease3::default();
        eye.set_duration(tuning.ease[0]);
        let mut look = Ease3::default();
        look.set_duration(tuning.ease[1]);
        let mut lean = Ease3::default();
        lean.set_duration(tuning.ease[3]);
        ChaseCamera {
            view,
            tuning,
            eye,
            look,
            lean,
            heading: Ease3::with_durations(tuning.heading_ease),
            fov: Ease { duration: tuning.ease[2], ..Default::default() },
            accel: [Vec3::ZERO; 4],
            frames: 0,
            last_velocity: None,
            age: 0.0,
            clock: None,
            trap_speed,
            nitrous_flame: false,
            since_ground: 0.0,
            fresh: true,
            impact: Vec3::ZERO,
            look_back: false,
            split: false,
            race: crate::race::RaceMode::FreeRoam,
        }
    }

    /// The camera back from its park (Camera_Factory 0x5F1FE0 with the
    /// keep flag parks the mover in player +0x23C, vtable +0x40 = 0x4440A0,
    /// and runs no update on it; 0x5FFE80 → 0x5FFDE0 at the event camera's
    /// end makes it current again: vtable +0x20 = 0x445B10 sets +0x52, then
    /// +0x14 the look-back, +0x3C = 0x445A50 re-attaches it). With +0x52
    /// the next update (0x453570 → 0x44DE90) snaps every ease to its target:
    /// eye +0x38, look-at +0x3C, field of view +0x34, lean +0x44, and the
    /// chase direction +0x40 through 0x445C20's second argument. **Verified**
    /// in the disassembly and **observed** under Wine (captures):
    /// the parked mover's pointer comes back in +0x238 and view 1 faces the
    /// car's heading within 0.1–0.2° on that first frame (lagging 2.7–7.8°
    /// before the crash). `parked_for`: the simulation time the mover stood
    /// parked; the ground stamp +0x58 is a clock (0x865188), so it counts.
    /// The acceleration history restarts from the next frame: the native
    /// filter (0x446F10) is fed the car's own +0x80, which has no gap, while
    /// the clone differentiates the velocity (**inferred** equivalent).
    pub fn resume(&mut self, parked_for: f32) {
        self.fresh = true;
        self.last_velocity = None;
        self.since_ground += parked_for.max(0.0);
    }

    /// The mover re-aimed after a stamped placement of its car (vtable
    /// +0x38 = 0x445B20, **verified**): every ease (eye +0x38, look-at
    /// +0x3C, field of view +0x34, lean +0x44, chase direction +0x40) takes
    /// its current target (value +0 = +8, slope +4 = +0xC, mode +0x2C = 0).
    /// The targets are the last update's, so the next one eases on from
    /// there towards the moved car's (the swing). The acceleration history
    /// restarts (the clone differentiates the velocity, whose jump at the
    /// placement is no acceleration; **inferred**).
    pub fn settle(&mut self) {
        self.eye.snap();
        self.look.snap();
        self.lean.snap();
        self.heading.snap();
        self.fov.snap();
        self.last_velocity = None;
    }

    /// Hold or release the look-back (0x445A30 stores mover +0x51; on a
    /// change it also sets 0x883E34 = 1 through 0x5D7AF0, whose reader is
    /// not traced). With +0x51 set, everything turns on the player's +0x2B
    /// (0x5EB530 returns 1 when it is clear; **verified**, 0x445EB0,
    /// 0x4539A5..0x4539BD, 0x445A60):
    ///
    /// * +0x2B set: the view keeps its own entry and 0x453570 multiplies
    ///   the eased eye and look-at point by −1 in x and y (0x7842A0) before
    ///   the frame transform: the eye at once on the mirrored side of the
    ///   car's origin, no ease; the walls push at 0.7 m (0x787090);
    /// * +0x2B clear: the entry is 0x7EC400's row (−17.2, 0, 6), not
    ///   mirrored (factor 1.0, 0x784250), eased; walls at 3 m (0x787094).
    ///
    /// The player's +0x2B read 1 under Wine (free roam, 2026-11-05;
    /// docs/notes/parity/world-systems-parity.md): the far chase looked back from 5.53 m
    /// ahead of the centre of mass, 1.36 m up, the bumper from 1.30 m
    /// behind it, both the first frame — the mirror of their own eyes about
    // prov: open chase.set_look_back
    /// the origin. Its writer is not traced (**open**), so the clone takes
    /// the mirror always.
    pub fn set_look_back(&mut self, on: bool) {
        self.look_back = on;
    }

    /// The race's mode: in drift (0x89E7D9) and URL (0x89E7E2) races the
    /// eye's acceleration lag is off (0x453570 zeroes the blended tuning's
    /// +0x40 scale); a drag race (0x89E7D8) keeps the shake's full
    /// amplitude, the others take 0.2 of it.
    pub fn set_race(&mut self, race: crate::race::RaceMode) {
        self.race = race;
    }

    /// The sim clock (0x865188, ticks of 1/4000 s), which the shake's noise
    /// runs on (0x452ED0 passes `(float)[0x865188] · 0.00025` to 0x441340;
    /// **verified**). Without it the camera's own age stands in (a phase
    /// difference only).
    pub fn set_clock(&mut self, ticks: u32) {
        self.clock = Some(ticks as f32 * 0.00025);
    }

    /// The impact shake's offset the next update applies (see `impact`):
    /// [`impact::ImpactShake::offset`] while the world's time scale is at
    /// least 1, else zero.
    pub fn set_impact(&mut self, offset: Vec3) {
        self.impact = offset;
    }

    /// Whether the car's nitrous burns this frame (0x452ED0: 0x59FA40 on
    /// car +0x40 non-zero and 0x5ECDC0 zero, i.e. a shot running with the
    /// gas on at 5 mph or more — `audio::car::nitrous::purging` false):
    /// the sway amplitude +0.2 and its rate ×1.4 (**verified**).
    pub fn set_nitrous_flame(&mut self, on: bool) {
        self.nitrous_flame = on;
    }

    /// The shake graph's speed scale (see `trap_speed`).
    pub fn trap_speed(&self) -> f32 {
        self.trap_speed
    }

    pub fn looking_back(&self) -> bool {
        self.look_back
    }

    /// The view leaves the car out (0x453570 sets view +0x280 to the car,
    /// which the car list's draw 0x60D584 skips): bumper and driver always,
    /// the hood while looking back; never the chase kinds.
    pub fn hides_car(&self) -> bool {
        !self.is_chase() && (self.look_back || self.view.kind != 4)
    }

    /// The radius walls push the eye out to (0x445A60): 3 m only while
    /// looking back through the 0x7EC400 entry (player +0x2B clear).
    pub fn wall_radius(&self) -> f32 {
        WALL_RADIUS[(self.look_back && LOOK_BACK_TABLE) as usize]
    }

    pub fn kind(&self) -> u16 {
        self.view.kind
    }

    /// [`ChaseCamera::is_chase`], for the screen layout (0x5C21F0 asks view
    /// 1's mover, vtable +0x24, before drawing the rear-view mirror).
    pub fn is_chase_kind(&self) -> bool {
        self.is_chase()
    }

    /// Far, near and high chase: the frame follows the direction of travel
    /// and the horizon leans (vtable slot 9, 0x445A90).
    fn is_chase(&self) -> bool {
        matches!(self.view.kind, 0 | 1 | 5)
    }

    /// Advance one rendered frame and return the camera to draw with.
    /// `fov_offset`: the Enhancements page's Field Of View offset
    /// (radians, 0 native) as the tick took it.
    pub fn update(&mut self, car: &CarState, dt: f32, fov_offset: f32) -> Camera {
        let dt = dt.max(1e-4);
        let rotation = Mat3::from_quat(car.orientation);
        let forward = rotation.x_axis;

        // The eases move first, towards last frame's targets (0x453570).
        if !self.fresh {
            self.eye.step(dt);
            self.look.step(dt);
            self.lean.step(dt);
            self.heading.step(dt);
            self.fov.step(dt);
        }
        self.age += dt;
        self.since_ground += dt;

        // Where the chase frame should face (0x445C20).
        let heading_target = if self.is_chase() {
            let mut v = car.velocity;
            let along = v.dot(forward);
            if along < 0.0 {
                // Reversing: mirror the velocity so the camera stays behind.
                v -= forward * (2.0 * along);
            }
            let (lo, hi) = FOLLOW_TRAVEL;
            let w = ((car.velocity.length() - lo) / (hi - lo)).clamp(0.0, 1.0);
            let mut d = v.normalize_or(forward) * w + forward * (1.0 - w);
            d.z *= (1.0 - self.since_ground / 2.0).clamp(0.0, 1.0);
            d
        } else {
            forward
        };
        self.heading.set_target(heading_target);
        // 0x44DF2B..0x44DF41: the stamp, after the chase direction.
        if car.wheels.iter().filter(|w| w.on_ground).count() > 1 {
            self.since_ground = 0.0;
        }
        if self.fresh {
            self.heading.snap();
        }

        // The frame the eye and look-at point live in (0x445E00).
        let frame = if self.is_chase() {
            let x = self.heading.value().normalize_or(forward);
            let y = Vec3::Z.cross(x).normalize_or(rotation.y_axis);
            Mat3::from_cols(x, y, x.cross(y))
        } else {
            rotation
        };

        // Acceleration, averaged over four frames, in that frame (0x446F10).
        let a = self.last_velocity.map_or(Vec3::ZERO, |v| (car.velocity - v) / dt);
        self.last_velocity = Some(car.velocity);
        self.accel[self.frames % 4] = a;
        self.frames += 1;
        let n = self.frames.min(4);
        let mean = self.accel[..n].iter().copied().sum::<Vec3>() / n as f32;
        let local = frame.transpose() * mean;

        // Targets (0x44DE90).
        // 0x445EB0: the look-back entry replaces the view's own only with
        // the player's +0x2B clear (never here, see `set_look_back`).
        let (e, tilt) = if self.look_back && LOOK_BACK_TABLE {
            LOOK_BACK[self.split as usize]
        } else {
            (self.view.eye, self.view.tilt)
        };
        let tilt = binary_angle(tilt).tan();
        let look_point =
            if self.is_chase() { Vec3::new(0.0, 0.0, e.x * tilt + e.z) } else { Vec3::new(e.x + 1.0, 0.0, e.z - tilt) };
        let mut eye_lag = self.tuning.eye;
        if self.race.drift() || self.race.url() {
            eye_lag.scale = Vec3::ZERO;
        }
        self.eye.set_target(eye_lag.apply(local, Vec3::ZERO) + e);
        self.look.set_target(self.tuning.look.apply(local, Vec3::ZERO) + look_point);
        self.lean.set_target(self.tuning.lean.apply(local, Vec3::Z));
        self.fov.target = binary_angle(FIELD_OF_VIEW[self.view.kind as usize % 6]);
        // Bumper and driver views are rigid: they snap every frame.
        if self.fresh || matches!(self.view.kind, 2 | 3) {
            self.eye.snap();
            self.look.snap();
            self.lean.snap();
            self.fov.snap();
        }
        self.fresh = false;

        // Looking back with the player's +0x2B set mirrors the eased points
        // in x and y (0x4539B5: 0x7842A0 = −1).
        let mirror = if self.look_back && !LOOK_BACK_TABLE { Vec3::new(-1.0, -1.0, 1.0) } else { Vec3::ONE };
        let mut eye = car.position + frame * (self.eye.value() * mirror);
        let mut target = car.position + frame * (self.look.value() * mirror);
        // The chase views lean by the eased x/y with z fixed at 1, in world
        // axes as the game does; the others take the car's up.
        let up = if self.is_chase() {
            let l = self.lean.value();
            Vec3::new(l.x, l.y, 1.0)
        } else {
            rotation.z_axis
        };
        // The impact shake (0x572A60 at 0x453B55) on the look-at's view,
        // before the speed shake turns that view.
        let mut up = up;
        if let Some((e, f, u)) = super::impact::shake_view(eye, target - eye, up, self.impact) {
            target = e + f * (target - eye).length();
            eye = e;
            up = u;
        }
        // Shake (0x452ED0 → 0x441340), in the camera's own axes.
        let surfaces = car.wheels.map(|w| w.surface);
        let sway = shake(
            car.velocity.length(),
            self.trap_speed,
            self.race.drag(),
            self.nitrous_flame,
            surfaces,
            self.clock.unwrap_or(self.age),
        );
        let forward = (target - eye).normalize_or(Vec3::X);
        let right = forward.cross(up).normalize_or(Vec3::Y);
        let cam_up = right.cross(forward);
        let offset = right * sway[0] + cam_up * sway[1];
        // Angles are scaled to the field of view against 60° (0x4412E0).
        let fov = self.fov.value;
        let scale = |deg: f32| {
            let a = deg.to_radians();
            (a.sin() * (fov * 0.5).sin() / 30f32.to_radians().sin()).clamp(-1.0, 1.0).asin()
        };
        let look = glam::Quat::from_axis_angle(cam_up, scale(sway[3]))
            * glam::Quat::from_axis_angle(right, scale(sway[2]))
            * (target - eye);
        eye += offset;
        target = eye + look;

        let mut camera = Camera::look_at(eye, target, up, self.fov.value);
        // 0x5C7ED0 widens and narrows the angle itself (see Camera::game_fov).
        // The Enhancements page's Field Of View offset (clone deviation,
        // 0 = native) widens the player views alone.
        camera.game_fov = Some(self.fov.value + fov_offset);
        camera
    }
}

/// The shake's four channels (side m, up m, pitch °, yaw °) at a time
/// (0x452ED0 → 0x441340, **verified**): the graph 0x7EC430 at row
/// `speed` (car +0x430) · 4 / `trap` (the set-up's +0x20; 0 gives an
/// infinite row, the last); the sway amplitude graph[0] · 0x7EBE90[kind]
/// (1.0) · 0.2 outside a drag race (0x89E7D8 / 0x890108), + 0.2 with a
/// burning nitrous shot, its rate graph[2] (× 1.4 with the flame); the
/// rough channel's amplitude graph[1] · Σ row +0x3C of the wheels'
/// surfaces · 10 · 0.25 · 0x7EBEA8[kind] (1.0; no drag factor), its rate
/// graph[3] · Σ row +0x38 · 0.25; per channel the six-octave noise of
/// rate · channel rate · time, times amplitude · channel amplitude, sway
/// plus rough (0x4414BD..0x4414F2).
pub(super) fn shake(speed: f32, trap: f32, drag: bool, flame: bool, surfaces: [u8; 4], time: f32) -> [f32; 4] {
    let scale = if trap > 0.0 { 4.0 / trap } else { f32::INFINITY };
    let row = (speed * scale).clamp(0.0, 4.0);
    let row = if row.is_nan() { 0.0 } else { row };
    let (i, t) = ((row as usize).min(3), row - (row as usize).min(3) as f32);
    let graph: [f32; 4] = std::array::from_fn(|k| SHAKE[i][k] * (1.0 - t) + SHAKE[i + 1][k] * t);
    let mut amplitude = graph[0] * if drag { 1.0 } else { 0.2 };
    let mut rate = graph[2];
    if flame {
        amplitude += 0.2;
        rate = graph[2] * 1.4;
    }
    let rows = surfaces.map(surface_shake);
    let rough_amplitude = graph[1] * rows.iter().map(|r| r.1).sum::<f32>() * 10.0 * 0.25;
    let rough_rate = graph[3] * rows.iter().map(|r| r.0).sum::<f32>() * 0.25;
    std::array::from_fn(|k| {
        fractal_noise(SWAY_RATE[k] * rate * time) * SWAY_AMPLITUDE[k] * amplitude
            + fractal_noise(ROUGH_RATE[k] * rough_rate * time) * ROUGH_AMPLITUDE[k] * rough_amplitude
    })
}
