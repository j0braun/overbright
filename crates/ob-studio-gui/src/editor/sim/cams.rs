//! The cameras on a recorded run: Follow (behind and above the chosen car,
//! held rigid) and Chase (the game's own in-race camera for that car's
//! model, its Far view). Both read the recorded frames only, so the view at
//! a time is the same whatever the display rate or the way there (played,
//! scrubbed, stepped).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use glam::Vec3;

use super::format::CarSample;
use super::record::Recording;
use crate::draw::camera::Camera;
use nfsu2::studio_api::car::Car;
use nfsu2::studio_api::chase::{CarState, ChaseCamera, DEFAULT_KIND, physics_car};

/// How the 3D camera rides with the chosen car.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CamMode {
    /// Behind and above the car, turning with it.
    #[default]
    Follow,
    /// The game's chase camera (the Far view the game starts on).
    Chase,
}

impl CamMode {
    pub const ALL: [CamMode; 2] = [CamMode::Follow, CamMode::Chase];

    pub fn label(self) -> &'static str {
        match self {
            CamMode::Follow => "Follow",
            CamMode::Chase => "Chase",
        }
    }

    pub fn tip(self) -> &'static str {
        match self {
            CamMode::Follow => "Behind and above the car, turning with it",
            CamMode::Chase => "The game's chase camera (Far view) as it rides with this car",
        }
    }
}

/// Recorded frames the chase camera is stepped over before the one shown
/// (3 s at the game's 60 Hz): its eases settle in well under that, so the
/// view at a time does not depend on how the playback got there.
pub const WARM_UP: usize = 180;

/// The Follow camera on car `c`: 14 m behind, 6 m up, looking at its roof.
pub fn follow(c: &CarSample) -> Camera {
    let h = c.heading();
    let back = Vec3::new(h.cos(), h.sin(), 0.0);
    let eye = c.position - back * 14.0 + Vec3::Z * 6.0;
    Camera::looking_at(eye, c.position + Vec3::Z * 1.0)
}

/// What the chase camera reads of car `id` in frame `k`: placed and turned
/// as recorded, moving as from the frame before (or its recorded speed along
/// its nose), all wheels down.
fn state(rec: &Recording, k: usize, id: u32) -> Option<CarState> {
    let f = &rec.frames[k];
    let c = f.cars.iter().find(|c| c.id == id)?;
    let before = k.checked_sub(1).and_then(|j| {
        let p = &rec.frames[j];
        let b = p.cars.iter().find(|x| x.id == id)?;
        let dt = f.time - p.time;
        (dt > 0.0).then(|| (c.position - b.position) / dt)
    });
    let mut s = CarState::new(c.position, 0.0);
    s.orientation = c.rotation;
    s.velocity = before.unwrap_or(c.rotation * Vec3::X * c.speed);
    for w in s.wheels.iter_mut() {
        w.on_ground = true;
    }
    Some(s)
}

/// The game's chase view of car `id` at `time`: a fresh camera stepped once
/// per recorded frame from [`WARM_UP`] frames before, at the frames' own
/// step; between two frames the two views blended. None before the first
/// frame or when the car is not in the frame.
pub fn chase(rec: &Recording, car: &Car, id: u32, time: f32) -> Option<Camera> {
    let f = &rec.frames;
    let k = f.partition_point(|x| x.time <= time);
    let a = k.checked_sub(1)?;
    let last = if k < f.len() && time > f[a].time { k } else { a };
    let mut cam = ChaseCamera::new(car, DEFAULT_KIND);
    let (mut at_a, mut at_last) = (None, None);
    let from = a.saturating_sub(WARM_UP);
    for j in from..=last {
        let Some(s) = state(rec, j, id) else { continue };
        let dt = if j > from { f[j].time - f[j - 1].time } else { 1.0 / 60.0 };
        let c = cam.update(&s, dt.max(1e-4), 0.0);
        if j == a {
            at_a = Some(c);
        }
        if j == last {
            at_last = Some(c);
        }
    }
    let a_cam = at_a?;
    if last == a {
        return Some(a_cam);
    }
    let b_cam = at_last?;
    let t = ((time - f[a].time) / (f[last].time - f[a].time)).clamp(0.0, 1.0);
    let turn = (b_cam.yaw - a_cam.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
    Some(Camera {
        position: a_cam.position.lerp(b_cam.position, t),
        yaw: a_cam.yaw + turn * t,
        pitch: a_cam.pitch + (b_cam.pitch - a_cam.pitch) * t,
        ..a_cam
    })
}

/// The cars the chase camera needs (each model's type: its camera views and
/// gearing), loaded once from the game folder; a model that does not load
/// stays None (the view then follows).
#[derive(Default)]
pub struct ChaseCars {
    cars: HashMap<(PathBuf, String), Option<Car>>,
}

impl ChaseCars {
    pub fn get(&mut self, root: &Path, model: &str) -> Option<&Car> {
        self.cars
            .entry((root.to_path_buf(), model.to_string()))
            .or_insert_with(|| match physics_car(root, model) {
                Ok(c) => Some(c),
                Err(e) => {
                    log::warn!("simulation chase camera: {model}: {e:#}");
                    None
                }
            })
            .as_ref()
    }
}

/// The 3D camera for car `id` at the time shown: Chase when its model
/// loads, else Follow; None when the car is not in the run at that time.
pub fn view(rec: &Recording, mode: CamMode, cars: &mut ChaseCars, root: &Path, id: u32, time: f32) -> Option<Camera> {
    if mode == CamMode::Chase
        && let Some(model) = rec.look_at(id, time).map(|l| l.model.clone())
        && let Some(car) = cars.get(root, &model)
        && let Some(c) = chase(rec, car, id, time)
    {
        return Some(c);
    }
    rec.cars_at(time).iter().find(|c| c.id == id).map(follow)
}
