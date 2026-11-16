//! A frame's world lighting for its cars (moved from render/lighting.rs).

use glam::{Mat4, Vec3};

use super::lighting::{CameraAxes, CarLighting, IN_GAME_RIG, RigLight, axes_lighting};
use crate::formats::lights::{LightPack, WorldLight};

/// The car-lighting options a frame's cars are lit with, read once per
/// frame: every layer of [`WorldLights::car`] is one of these, so
/// the chain is one function of the options (all off: the native car
/// lighting, 0x620AF0's).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CarLightOptions {
    /// The higher Enhanced settings or ray tracing (`qol::scene_car_specular`,
    /// the rule `qol::scene_lights_car`): the highlight from the
    /// scene, not the camera rig (docs/car-scene-specular.md).
    pub scene_specular: bool,
    /// Dynamic Lamps: the light-pack lamps per pixel.
    pub dynamic_lamps: bool,
    /// Headlight Beams Next-Gen: the other cars' lights on the body.
    pub car_lights_next_gen: bool,
    /// Light Probes: the probes' hue in the ambient light.
    pub light_probes: bool,
}

impl CarLightOptions {
    /// The options of the frame's settings `opts`.
    pub fn of(opts: &crate::render::RenderOptions) -> CarLightOptions {
        CarLightOptions {
            scene_specular: opts.scene_car_specular,
            dynamic_lamps: opts.dynamic_lamps,
            car_lights_next_gen: opts.car_lights_next_gen,
            light_probes: opts.light_probes,
        }
    }
}

/// A frame's world lighting for its cars ([`CarLighting::world_lights`]).
pub struct WorldLights<'a> {
    packs: Vec<&'a LightPack>,
    /// The packs' lights for Dynamic Lamps (None: the option is off).
    lamps: Option<Vec<&'a WorldLight>>,
    scene: &'a [WorldLight],
    axes: CameraAxes,
    eye: Vec3,
    options: CarLightOptions,
}

impl WorldLights<'_> {
    /// A car at `car_world`: [`CarLighting::in_world`]'s values. The layers,
    /// in order: the native harmonic (rig + light packs; with the scene
    /// specular its highlight from the environment and the lamps),
    /// the four per-pixel lamps (Dynamic Lamps, replaced by Modern
    /// Lighting's, then by Next-Gen's car lights), the probes' ambient.
    pub fn car(&self, car_world: Mat4) -> CarLighting {
        let _t = crate::prof::acc::span("cars: lighting");
        let o = self.options;
        let at = car_world.w_axis.truncate();
        let mut lighting = CarLighting::compute_with(
            &IN_GAME_RIG,
            &self.packs,
            self.scene,
            &self.axes,
            self.eye,
            car_world,
            false,
            o.scene_specular,
        );
        if let Some(lights) = &self.lamps {
            lighting.lamps = crate::render::lamps::car_lamps(lights, at);
        }
        // Modern Lighting: the same lights as the world's.
        if let Some(l) = crate::render::modern_light_bold::car_lamps(at) {
            lighting.lamps = l;
        }
        // Next-Gen car lights (docs/car-lights.md): the other cars'
        // lights and the car's own underglow among its four lamps (the
        // native-weight form; Modern Lighting's form is left as it is).
        if o.car_lights_next_gen && lighting.lamps[1][3] >= 0.0 {
            let cars = crate::render::car_lights::cars(o.car_lights_next_gen);
            if !cars.is_empty() {
                lighting.lamps =
                    crate::render::car_lights::car_lamps(&cars, at, lighting.lamps, &crate::render::car_lights::look());
            }
        }
        // Light Probes (docs/lighting2.md): the nearest baked
        // probes' hue replaces the harmonic's ambient colour on each side,
        // energy-bounded (rgb only), then into the car's axes.
        if !o.light_probes {
            return lighting;
        }
        if crate::render::probes::log_on() {
            log::warn!("car at {} probe cube {:?}", at, crate::render::probes::cube_at(at, o.light_probes));
        }
        if let Some(cube) = crate::render::probes::cube_at(at, o.light_probes) {
            let terms = crate::render::probes::ambient_terms(&cube, &lighting.world_harmonic);
            for (h, t) in lighting.world_harmonic.iter_mut().zip(terms) {
                *h += t;
            }
            (lighting.harmonic, lighting.local_eye) =
                axes_lighting(&lighting.world_harmonic, lighting.eye, car_world, lighting.frontend);
        }
        lighting
    }
}

impl CarLighting {
    /// A car in a loaded world, seen by `camera`: the in-game setup and
    /// the light packs of the streamed-in sections. 0x48E3D0 walks the list
    /// 0x830D14, which holds only the loaded sections' packs (0x48E0B0
    /// links a section's 0x80135000 pack in as the section loads); the
    /// clone stands the camera section's visible list in for the loaded
    /// set, as the world draw does, the last one while the camera is
    /// outside every section ([`crate::world::World::light_pack_sections`]).
    /// (Checked against an apitrace of the original at the airport start: its HarmonicCoeff is the white rig
    /// alone, without section 121's far "moon" light (40, 40, 55, radius
    /// 9999.99) that a search of every pack picks up.)
    pub fn in_world(world: &crate::world::World, camera: &crate::render::Camera, car_world: Mat4, opts: &crate::render::RenderOptions) -> CarLighting {
        CarLighting::world_lights(world, camera, opts).car(car_world)
    }

    /// What [`CarLighting::in_world`] takes of the world and the camera,
    /// the same for every car of a frame (the packs of the camera
    /// section's visible list): worked out once, then [`WorldLights::car`]
    /// per car gives what `in_world` gives.
    pub fn world_lights<'a>(world: &'a crate::world::World, camera: &crate::render::Camera, opts: &crate::render::RenderOptions) -> WorldLights<'a> {
        let axes = CameraAxes::look(camera.forward(), camera.up);
        let packs: Vec<&LightPack> = {
            let loaded = world.light_pack_sections(camera.position);
            // The loaded set as bits (section numbers stay below 4096).
            let mut bits = [0u64; 64];
            let mut beyond = Vec::new();
            for &s in loaded {
                match bits.get_mut(s as usize / 64) {
                    Some(b) => *b |= 1 << (s % 64),
                    None => beyond.push(s),
                }
            }
            let has =
                |s: u32| bits.get(s as usize / 64).map_or_else(|| beyond.contains(&s), |b| b >> (s % 64) & 1 != 0);
            world.light_packs.iter().filter(|p| has(p.section)).collect()
        };
        let options = CarLightOptions::of(opts);
        let lamps = options.dynamic_lamps.then(|| packs.iter().flat_map(|p| p.lights.iter()).collect());
        WorldLights { packs, lamps, scene: &world.scene_lights, axes, eye: camera.position, options }
    }

    /// A car on a frontend stage (no track lights).
    pub fn frontend(rig: &[RigLight; 3], camera: &crate::render::Camera, car_world: Mat4) -> CarLighting {
        let axes = CameraAxes::look(camera.forward(), camera.up);
        CarLighting::compute(rig, &[], &[], &axes, camera.position, car_world, true)
    }
}
