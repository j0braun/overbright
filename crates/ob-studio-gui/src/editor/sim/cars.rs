//! The simulation's cars drawn as the game draws them: each car's look
//! from the stream (its model and fitted visual parts, `L` records) is
//! built with the game library's own car loader and drawn with the game's
//! car renderer over the world, between the 60 Hz ticks, with its wheels
//! (steer, roll, lift) and every car's headlight pool on the road. While a
//! car loads, or when its model cannot be built (no game data), the view
//! keeps the outline box.
//!
//! The frame hook puts the cars of the time shown into
//! [`EditorState::replay`] ([`Replay::set`]); the viewport draws them right
//! after the world ([`CarDraw::draw`]) and says which it drew, so the
//! overlay leaves their boxes out.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc;

use glam::{Mat4, Vec3};

use super::format::{CarLook, CarSample, LIGHT_HEAD, Wheels};
use crate::render::car::{CarLighting, CarRenderer};
use crate::render::{Gpu, HeadlightPool, RenderOptions};
use nfsu2::studio_api::car::{Car, WheelPose};

/// Cars built at once on their own threads.
const LOADING_AT_ONCE: usize = 4;

/// One car of the time shown.
#[derive(Clone, Debug, PartialEq)]
pub struct ReplayCar {
    pub id: u32,
    pub look: CarLook,
    /// Car space → world.
    pub transform: Mat4,
    pub wheels: Option<Wheels>,
}

/// The cars the viewport draws this frame, kept in the editor's state.
#[derive(Clone, Debug)]
pub struct Replay {
    /// Car models (the default); off: outline boxes, as before.
    pub models: bool,
    pub cars: Vec<ReplayCar>,
    /// Bumped whenever `cars` or `models` change, so the view redraws.
    pub version: u64,
    /// The cars the last frame drew as models (the overlay boxes the rest).
    pub drawn: HashSet<u32>,
    /// Models still loading (the window keeps drawing until they are in).
    pub loading: bool,
}

impl Default for Replay {
    fn default() -> Replay {
        Replay { models: true, cars: Vec::new(), version: 0, drawn: HashSet::new(), loading: false }
    }
}

impl Replay {
    /// This frame's cars (none: nothing drawn).
    pub fn set(&mut self, cars: Vec<ReplayCar>) {
        if cars != self.cars {
            self.cars = cars;
            self.version += 1;
        }
    }

    pub fn set_models(&mut self, on: bool) {
        if on != self.models {
            self.models = on;
            self.version += 1;
        }
    }

    /// Whether the car `id` shows as a model (no box needed).
    pub fn shows(&self, id: u32) -> bool {
        self.models && self.drawn.contains(&id)
    }
}

/// The cars of `samples` that have a look in the recording.
pub fn replay_cars(rec: &super::record::Recording, time: f32, samples: &[CarSample]) -> Vec<ReplayCar> {
    samples
        .iter()
        .filter_map(|c| {
            let look = rec.look_at(c.id, time)?;
            Some(ReplayCar {
                id: c.id,
                look: look.clone(),
                transform: Mat4::from_rotation_translation(c.rotation, c.position),
                wheels: c.wheels,
            })
        })
        .collect()
}

/// The wheels as the car renderer takes them: the recorded steer, roll and
/// lift; the front wheels' and brakes' x-turn from the look (as
/// `CarState::wheel_poses` sets it); spinners at rest.
pub fn wheel_poses(wheels: Option<&Wheels>, camber: f32) -> [WheelPose; 4] {
    std::array::from_fn(|i| match wheels {
        Some(w) => WheelPose {
            lift: w.lift[i],
            steer: if i < 2 { w.steer[i] } else { 0.0 },
            roll: w.roll[i],
            tilt: camber,
            brake_tilt: camber,
            ..Default::default()
        },
        None => WheelPose { tilt: camber, brake_tilt: camber, ..Default::default() },
    })
}

/// A look without the car it is on: what builds the model.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Key {
    model: String,
    camber_bits: u32,
    parts: Vec<(u8, u32)>,
}

impl Key {
    fn of(l: &CarLook) -> Key {
        Key { model: l.model.clone(), camber_bits: l.camber_bits, parts: l.parts.clone() }
    }
}

enum Built {
    Loading(mpsc::Receiver<Result<Car, String>>),
    Waiting,
    Ready(Arc<Car>),
    Failed,
}

/// Build the car of `key` from the game folder (the game library's own
/// loader: the model, its parts, paint, vinyls, rims and lights).
fn build(root: &Path, key: &Key) -> Result<Car, String> {
    let parts = key.parts.iter().copied().collect();
    let mut car = Car::load_with(root, &key.model, &parts).map_err(|e| format!("{}: {e:#}", key.model))?;
    car.camber = f32::from_bits(key.camber_bits);
    Ok(car)
}

/// The cars' models (one per look) and renderers (one per car: each holds
/// its own pose for the frame).
pub struct CarDraw {
    root: PathBuf,
    built: HashMap<Key, Built>,
    renderers: HashMap<u32, (Key, CarRenderer)>,
    /// Load each model before drawing (`--shot`: the same picture every
    /// run), not on threads.
    pub blocking: bool,
}

impl CarDraw {
    pub fn new(root: PathBuf) -> CarDraw {
        CarDraw { root, built: HashMap::new(), renderers: HashMap::new(), blocking: false }
    }

    /// Start (or with `blocking`, do) the loads `cars` need; take finished
    /// ones. True when a model came in since the last call.
    pub fn update(&mut self, cars: &[ReplayCar]) -> bool {
        for c in cars {
            let key = Key::of(&c.look);
            self.built.entry(key).or_insert(Built::Waiting);
        }
        let mut arrived = false;
        let mut loading = 0;
        for b in self.built.values_mut() {
            if let Built::Loading(rx) = b {
                match rx.try_recv() {
                    Ok(r) => {
                        *b = match r {
                            Ok(car) => Built::Ready(Arc::new(car)),
                            Err(e) => {
                                log::warn!("simulation car: {e}");
                                Built::Failed
                            }
                        };
                        arrived = true;
                    }
                    Err(mpsc::TryRecvError::Empty) => loading += 1,
                    Err(mpsc::TryRecvError::Disconnected) => *b = Built::Failed,
                }
            }
        }
        // The cars of this frame first, in their order.
        for c in cars {
            let key = Key::of(&c.look);
            let Some(b) = self.built.get_mut(&key) else { continue };
            if !matches!(b, Built::Waiting) {
                continue;
            }
            if self.blocking {
                *b = match build(&self.root, &key) {
                    Ok(car) => Built::Ready(Arc::new(car)),
                    Err(e) => {
                        log::warn!("simulation car: {e}");
                        Built::Failed
                    }
                };
                arrived = true;
            } else if loading < LOADING_AT_ONCE {
                let (tx, rx) = mpsc::channel();
                let (root, k) = (self.root.clone(), key.clone());
                let spawned = std::thread::Builder::new().name("sim car".into()).spawn(move || {
                    let _ = tx.send(build(&root, &k));
                });
                *b = if spawned.is_ok() { Built::Loading(rx) } else { Built::Failed };
                loading += 1;
            }
        }
        arrived
    }

    /// Whether every car of `cars` is built (or cannot be).
    pub fn settled(&self, cars: &[ReplayCar]) -> bool {
        cars.iter().all(|c| matches!(self.built.get(&Key::of(&c.look)), Some(Built::Ready(_) | Built::Failed)))
    }

    /// The headlight pools of the cars with a model (every car's
    /// headlights are on in game; a stream without lights: on too).
    pub fn pools(&self, cars: &[ReplayCar]) -> Vec<HeadlightPool> {
        cars.iter()
            .filter(|c| c.wheels.is_none_or(|w| w.lights & LIGHT_HEAD != 0))
            .filter_map(|c| match self.built.get(&Key::of(&c.look)) {
                Some(Built::Ready(car)) => {
                    Some(HeadlightPool { transform: c.transform, color: car.headlight_pool_color })
                }
                _ => None,
            })
            .collect()
    }

    /// Draw the cars with a model over the world just drawn into
    /// `color` / `depth` (its depth kept), seen from `camera`. Returns the
    /// ids drawn.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        world: &crate::world::World,
        camera: &crate::draw::camera::Camera,
        aspect: f32,
        cars: &[ReplayCar],
        opts: &RenderOptions,
    ) -> HashSet<u32> {
        let mut drawn = HashSet::new();
        // A car gone from the view, or on another look: its renderer goes.
        let here: HashMap<u32, Key> = cars.iter().map(|c| (c.id, Key::of(&c.look))).collect();
        self.renderers.retain(|id, (k, _)| here.get(id) == Some(k));
        if cars.is_empty() {
            return drawn;
        }
        let lights = CarLighting::world_lights(world, camera, opts);
        let (view_proj, view) = (camera.view_projection(aspect), camera.view());
        for c in cars {
            let key = Key::of(&c.look);
            let Some(Built::Ready(car)) = self.built.get(&key) else { continue };
            let (_, r) =
                self.renderers.entry(c.id).or_insert_with(|| (key.clone(), CarRenderer::new(gpu, car, format, opts)));
            let poses = wheel_poses(c.wheels.as_ref(), car.camber);
            r.render(gpu, encoder, color, depth, view_proj, view, &lights.car(c.transform), c.transform, &poses, opts);
            drawn.insert(c.id);
        }
        drawn
    }

    /// Free the renderers (the GPU side); the models stay.
    pub fn release(&mut self) {
        self.renderers.clear();
    }
}

/// Where a car sits for a box: its position (for the tests and labels).
pub fn position(c: &ReplayCar) -> Vec3 {
    c.transform.w_axis.truncate()
}

#[cfg(test)]
mod tests {
    use super::super::format::{CarKind, Frame, LIGHT_BRAKE, Line};
    use super::super::record::Recording;
    use super::*;
    use glam::Quat;

    fn look(id: u32, model: &str) -> CarLook {
        CarLook { id, model: model.into(), camber_bits: 0.05f32.to_bits(), parts: vec![(0x3f, 0x1234)] }
    }

    fn sample(id: u32, x: f32) -> CarSample {
        CarSample {
            id,
            kind: CarKind::Racer,
            position: Vec3::new(x, 0.0, 1.0),
            rotation: Quat::from_rotation_z(0.5),
            speed: 0.0,
            lap: 0,
            checkpoint: 0,
            place: 0,
            wheels: Some(Wheels {
                steer: [0.1, 0.09],
                roll: [1.0; 4],
                lift: [0.02; 4],
                lights: LIGHT_HEAD | LIGHT_BRAKE,
            }),
        }
    }

    #[test]
    fn cars_with_a_look_are_drawn_where_the_recording_has_them() {
        let mut rec = Recording::default();
        rec.push(Line::Look(look(1, "RX8")));
        rec.push(Line::Frame(Frame { tick: 0, time: 0.0, cars: vec![sample(1, 3.0), sample(2, 9.0)] }));
        let cars = replay_cars(&rec, 0.0, &rec.cars_at(0.0));
        // Car 2 has no look (an older stream): it stays a box.
        assert_eq!(cars.len(), 1);
        assert_eq!(cars[0].look.model, "RX8");
        assert!((position(&cars[0]) - Vec3::new(3.0, 0.0, 1.0)).length() < 1e-5);
        let poses = wheel_poses(cars[0].wheels.as_ref(), cars[0].look.camber());
        assert_eq!((poses[0].steer, poses[1].steer, poses[2].steer), (0.1, 0.09, 0.0));
        assert_eq!((poses[3].roll, poses[3].lift, poses[0].tilt), (1.0, 0.02, 0.05));
    }

    #[test]
    fn the_view_redraws_only_when_the_cars_change() {
        let mut r = Replay::default();
        assert!(r.models, "models are the default");
        let c = ReplayCar { id: 1, look: look(1, "RX8"), transform: Mat4::IDENTITY, wheels: None };
        r.set(vec![c.clone()]);
        let v = r.version;
        r.set(vec![c.clone()]);
        assert_eq!(r.version, v);
        r.set_models(false);
        assert_eq!(r.version, v + 1);
        r.drawn.insert(1);
        assert!(!r.shows(1), "boxes chosen");
        r.set_models(true);
        assert!(r.shows(1) && !r.shows(2));
    }

    /// With the game's files: a racer's look as a simulation streams it
    /// (model, paint and body parts) and a stock traffic car build with the
    /// game's loader, and both throw a headlight pool.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn the_games_cars_build_from_their_looks() {
        let mut d = CarDraw::new(crate::testkit::game_root());
        d.blocking = true;
        let racer = CarLook {
            id: 1,
            model: "PEUGOT".into(),
            camber_bits: 0,
            parts: vec![(0x0d, 0x1ee4_bd0f), (0x1b, 0xf56f_dc46)],
        };
        let traffic = CarLook { id: 100, model: "TAXI".into(), camber_bits: 0, parts: Vec::new() };
        let cars: Vec<ReplayCar> = [racer, traffic]
            .into_iter()
            .map(|look| ReplayCar { id: look.id, look, transform: Mat4::IDENTITY, wheels: None })
            .collect();
        assert!(d.update(&cars));
        assert!(d.settled(&cars));
        assert_eq!(d.pools(&cars).len(), 2, "both built");
    }

    #[test]
    fn a_model_that_cannot_be_built_settles_as_a_box() {
        let mut d = CarDraw::new(PathBuf::from("/nonexistent game folder"));
        d.blocking = true;
        let c = ReplayCar { id: 1, look: look(1, "NOSUCHCAR"), transform: Mat4::IDENTITY, wheels: None };
        assert!(d.update(std::slice::from_ref(&c)));
        assert!(d.settled(std::slice::from_ref(&c)));
        assert!(d.pools(std::slice::from_ref(&c)).is_empty());
    }
}
