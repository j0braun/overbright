//! The garage's own scene between two ticks (docs/uncapped-fps.md, "The
//! garage scene between ticks"): the turntable, its camera, the FE car's
//! hydraulics and part animations and the scene's particles, as each tick
//! leaves them (`GarageSnapshot`, captured with the world view's tick
//! snapshot), and a drawn frame's values between the last two
//! (`GarageDrawn`, `Game::frame_garage`). `render_garage` draws those in
//! place of the tick's own; at α ≥ 1 (every 60 Hz frame of the harness) it
//! draws the tick's state exactly as before.
use crate::*;
use nfsu2::render::native::fx::tick::{lerp, lerp_n};

/// The FE car's part poses (`GarageScene::car_poses`).
type PartPoses = [[Option<nfsu2::part_anim::ChannelPose>; 2]; 4];

/// What `render_garage` takes from one tick's garage.
#[derive(Clone, Debug, Default)]
pub(crate) struct GarageSnapshot {
    /// The view as drawn at the tick (`GarageRequest::view` at its `now`);
    /// None: no garage up (or its platform not loaded).
    pub(crate) view: Option<nfsu2::garage::View>,
    /// The platform and the FE car shown (name, parts, performance
    /// parts), hashed: frames blend only within one of each.
    pub(crate) key: u64,
    /// The request's time (s): a request made after the tick (a harness's
    /// own `garage_request`) shows as it is.
    pub(crate) now: f32,
    /// The FE car's place on the turntable (`GarageScene::car_world`).
    pub(crate) car_world: Option<glam::Mat4>,
    /// The hydraulics' corner heights and wheel drops (`Garage::corner`,
    /// `Garage::wheel_drop`).
    pub(crate) hydro: Option<([f32; 4], [f32; 4])>,
    /// The hood, trunk and doors (`GarageScene::car_poses`).
    pub(crate) poses: Option<PartPoses>,
    /// The request's camber and Lights-list flares.
    pub(crate) camber: f32,
    pub(crate) headlights: bool,
    /// The scene's particles (`GarageScene::fx`).
    pub(crate) particles: Vec<nfsu2::particles::Particle>,
}

/// One drawn frame's garage values between two ticks.
#[derive(Clone, Debug)]
pub(crate) struct GarageDrawn {
    /// The view: camera, car angle and steer, platform angle and disc.
    pub(crate) view: nfsu2::garage::View,
    pub(crate) car_world: Option<glam::Mat4>,
    /// The wheels' drops (`Garage::wheel_drop`).
    pub(crate) drops: Option<[f32; 4]>,
    pub(crate) poses: Option<PartPoses>,
    pub(crate) camber: f32,
    pub(crate) headlights: bool,
    /// The particles and their size factors (`nfsu2::particles::between`).
    pub(crate) particles: Vec<nfsu2::particles::Particle>,
    pub(crate) scale: Vec<f32>,
}

/// The platform and FE car `g` shows for `request`, hashed.
fn garage_key(g: &GarageScene, request: &GarageRequest) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    request.platform.hash(&mut h);
    g.platform.name.hash(&mut h);
    g.car.as_ref().map(|(n, installed, ..)| (n, installed)).hash(&mut h);
    g.car_perf.hash(&mut h);
    h.finish()
}

/// Two part poses `alpha` of the way (each channel blended where both
/// ticks pose it, else the older tick's).
fn poses_between(a: &PartPoses, b: &PartPoses, alpha: f32) -> PartPoses {
    std::array::from_fn(|c| {
        std::array::from_fn(|k| match (a[c][k], b[c][k]) {
            (Some(p), Some(q)) => Some(nfsu2::part_anim::ChannelPose {
                rotation: if p.rotation == q.rotation { p.rotation } else { p.rotation.slerp(q.rotation, alpha) },
                moved: lerp3(p.moved, q.moved, alpha),
                rest: lerp3(p.rest, q.rest, alpha),
                value: lerp(p.value, q.value, alpha),
            }),
            (p, _) => p,
        })
    })
}

fn lerp3(a: Vec3, b: Vec3, alpha: f32) -> Vec3 {
    Vec3::from_array(lerp_n(a.to_array(), b.to_array(), alpha))
}

/// The view `alpha` of the way from `a` to `b` (same disc): the camera's
/// field of view, target and eye, the car's angle and steer and the
/// platform's angle blended.
fn view_between(a: &nfsu2::garage::View, b: &nfsu2::garage::View, alpha: f32) -> nfsu2::garage::View {
    let mut v = *b;
    v.camera.fov = lerp(a.camera.fov, b.camera.fov, alpha);
    v.camera.target = lerp3(a.camera.target, b.camera.target, alpha);
    v.camera.eye = lerp3(a.camera.eye, b.camera.eye, alpha);
    v.camera.car_angle = lerp(a.camera.car_angle, b.camera.car_angle, alpha);
    v.camera.steer = lerp(a.camera.steer, b.camera.steer, alpha);
    v.car_angle = lerp(a.car_angle, b.car_angle, alpha);
    v.steer = lerp(a.steer, b.steer, alpha);
    v.platform_angle = lerp(a.platform_angle, b.platform_angle, alpha);
    v
}

impl Game {
    /// The tick's garage into `s` (a pure read; the particle buffer
    /// reused). Called at the end of every tick and, before a tick, by
    /// `refresh_snapshot` (a change outside the ticks is where the next
    /// tick's blend starts).
    pub(crate) fn capture_garage(&self, s: &mut GarageSnapshot) {
        s.particles.clear();
        let request = self.garage_fe.request.as_ref();
        let g = self.render.garage.as_ref().zip(request).filter(|(g, r)| g.platform.name == r.platform);
        let Some((g, request)) = g else {
            (s.view, s.key, s.car_world, s.hydro, s.poses) = (None, 0, None, None, None);
            return;
        };
        s.view = Some(request.view.at(request.now));
        s.key = garage_key(g, request);
        s.now = request.now;
        s.car_world = g.car_world;
        s.hydro = g.hydro.as_ref().map(|(h, _)| (h.corner, std::array::from_fn(|i| h.wheel_drop(i))));
        s.poses = g.car_poses;
        s.camber = request.camber;
        s.headlights = request.headlights;
        if let Some(fx) = g.fx.as_ref() {
            s.particles.extend_from_slice(&fx.0.particles);
        }
    }

    /// A drawn frame's garage `alpha` (0 ≤ α < 1) of the way from S_{n−1}
    /// to S_n (a pure read); None: draw the tick's own (α ≥ 1, no garage).
    /// Cuts (the tick's own drawn): no garage in S_{n−1} (entering it, the
    /// drive-in's handover), another platform or FE car (a car change, a
    /// part change's new car, a lot's step), the car on another disc (a
    /// car change's platform turn starting), a request or scene written
    /// after the tick. Each value is exactly S_{n−1}'s at α = 0.
    pub(crate) fn frame_garage(&self, alpha: f32) -> Option<GarageDrawn> {
        if !(alpha < 1.0) {
            return None;
        }
        let (prev, cur) = (&self.interp.prev.garage, &self.interp.cur.garage);
        let (Some(a), Some(b)) = (prev.view, cur.view) else { return None };
        let request = self.garage_fe.request.as_ref()?;
        let g = self.render.garage.as_ref().filter(|g| g.platform.name == request.platform)?;
        let fresh = cur.key == garage_key(g, request)
            && cur.now.to_bits() == request.now.to_bits()
            && cur.car_world.map(|m| m.to_cols_array().map(f32::to_bits))
                == g.car_world.map(|m| m.to_cols_array().map(f32::to_bits));
        if !fresh || prev.key != cur.key || a.disc != b.disc {
            return None;
        }
        let alpha = alpha.max(0.0);
        let view = view_between(&a, &b, alpha);
        let (drops, car_world) = match (prev.hydro, cur.hydro, prev.car_world, cur.car_world) {
            _ if alpha == 0.0 => (prev.hydro.map(|h| h.1), prev.car_world),
            (Some(p), Some(c), Some(_), Some(_)) => {
                let corner = lerp_n(p.0, c.0, alpha);
                let world = g
                    .car
                    .as_ref()
                    .map(|(_, _, car, _)| view.turntable().car_transform_on(&car.car_type.wheels, corner));
                (Some(lerp_n(p.1, c.1, alpha)), world)
            }
            _ => (prev.hydro.map(|h| h.1), prev.car_world),
        };
        let poses = match (prev.poses, cur.poses) {
            (Some(p), Some(c)) => Some(poses_between(&p, &c, alpha)),
            (p, _) => p,
        };
        let (mut particles, mut scale) = (Vec::new(), Vec::new());
        nfsu2::particles::between(&prev.particles, &cur.particles, alpha, |_, _| None, &mut particles, &mut scale);
        Some(GarageDrawn {
            view: if alpha == 0.0 { a } else { view },
            car_world,
            drops,
            poses,
            camber: prev.camber,
            headlights: prev.headlights,
            particles,
            scale,
        })
    }
}
