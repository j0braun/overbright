//! Weather effects hosted by the world renderer (moved from render/mod.rs).
//!
//! Enhanced effects on ticks (docs/uncapped-fps.md, "Enhanced effects on ticks"): the
//! effects' simulations step once per tick ([`WorldRenderer::enh_tick`],
//! called by the game's tick hook with the tick's state, [`EnhTick`]); a
//! drawn frame only reads what the last tick left (and applies the ticks'
//! commits to its GPU caches in order).

use crate::render::*;

/// What one tick hands the world renderer's Enhanced effects (built by
/// the game's tick hook from the tick's own state: the cars' poses, the
/// camera, the weather and the clocks at the end of the tick).
#[derive(Clone)]
pub struct EnhTick {
    /// The tick's index (from boot; the sky's row phase).
    pub tick: u64,
    /// The tick's dt (s).
    pub dt: f32,
    /// The tick's camera (the drive's, shaken).
    pub camera: Camera,
    /// The weather as the cars and the spray read it (None: no weather
    /// row on, or out of game).
    pub scene: Option<wet_car::Scene>,
    /// The road's wetness (`Weather::road_wet`; None out of game): what
    /// Ambient Detail's steam and dust follow.
    pub road_wet: Option<f32>,
    /// The player's speed (m/s): the lens drops' wind.
    pub speed: f32,
    /// The draw's shader clock (s) as the next drawn frame reads it
    /// (`Game::seconds`): Ambient Detail's and the sky's time.
    pub time: f32,
    /// The cars as the spray sees them, in drawing order (`CarSample`).
    pub cars: Vec<weather_fx::CarSample>,
    /// The cars' lights and wheels (Wet Road Interaction), as the glow
    /// pass handed them.
    pub extras: Vec<wet_road::CarExtra>,
    /// The cars for Ambient Detail.
    pub ambient: Vec<ambient_detail::CarIn>,
    /// The sky's state at the tick (the Next-Gen sky's panorama rows).
    pub sky: Option<sky_ng::SkyTick>,
    /// The settings the tick stepped with.
    pub opts: EnhTickOptions,
}

/// The rows the effects' tick reads, as the tick took them (the game's
/// tick settings).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EnhTickOptions {
    /// Any weather row on.
    pub weather_fx: bool,
    /// Wet Road Interaction (0 off).
    pub wet_interaction: u8,
    /// Rain Detail (0 off .. 3 Next-Gen).
    pub rain_detail: u8,
    /// Spray & Splashes.
    pub spray: bool,
    /// Lens Drops.
    pub lens_drops: bool,
    /// Ambient Detail (0 Off).
    pub ambient_detail: u8,
    /// Sky (0 Original).
    pub sky: u8,
}

impl EnhTickOptions {
    /// The frame's settings' rows.
    pub fn of_frame(opts: &RenderOptions) -> EnhTickOptions {
        EnhTickOptions {
            weather_fx: opts.weather_fx,
            wet_interaction: opts.wet_interaction,
            rain_detail: opts.rain_detail,
            spray: opts.spray,
            lens_drops: opts.lens_drops,
            ambient_detail: opts.ambient_detail,
            sky: opts.sky,
        }
    }
}

/// The weather effects' CPU half, stepped on the tick
/// ([`WorldRenderer::enh_tick`]); the draw reads it ("stepped" class:
/// the last tick's state) and applies the trail map's commits.
#[derive(Default)]
pub struct WeatherSim {
    /// The spray pool (Spray & Splashes, Wet Road Interaction's spray).
    pub spray: weather_fx::Spray,
    /// The camera's position at the last world clock step, and its
    /// smoothed velocity (the streaks' motion).
    last_eye: Option<(f32, glam::Vec3)>,
    pub cam_vel: glam::Vec3,
    /// Wet Road Interaction's trail map, its CPU half.
    pub trail: wet_road::TrailSim,
    /// The last tick's cars (the spray's samples) and the extras the
    /// spray and the road read at it: the glow pass's list one tick late,
    /// as the draw's push took them a frame late before (kept so a steady
    /// 60 Hz frame reads what it read).
    pub cars: Vec<weather_fx::CarSample>,
    pub extras: Vec<wet_road::CarExtra>,
    next_extras: Vec<wet_road::CarExtra>,
    /// The rows the last tick stepped with (None: no tick yet).
    pub stepped: Option<EnhTickOptions>,
}

impl WeatherSim {
    /// The rows the drawn weather state was stepped with: the last tick's,
    /// or (no tick stepped it) the frame's. A row changed between a tick
    /// and a frame is drawn from the next tick on, with the state stepped
    /// with it.
    pub fn stepped_or(&self, opts: &RenderOptions) -> EnhTickOptions {
        self.stepped.unwrap_or_else(|| EnhTickOptions::of_frame(opts))
    }

    /// A digest of the spray, the camera velocity and the trail map's CPU
    /// half (the rate tests compare it at equal ticks).
    pub fn digest(&self) -> u64 {
        let mut h = self.spray.digest() ^ self.trail.digest().rotate_left(17);
        for v in [self.cam_vel.x, self.cam_vel.y, self.cam_vel.z] {
            h = (h ^ u64::from(v.to_bits())).wrapping_mul(0x100_0000_01b3);
        }
        h
    }
}

impl WorldRenderer {
    /// The tick: step the Enhanced effects' simulations once,
    /// with the tick's state: the spray (and the camera's velocity), the
    /// eye's share of it, the lens drops, the trail map's commit, Ambient
    /// Detail's debris and volume steps, the sky's panorama rows. Nothing
    /// here touches the GPU.
    pub fn enh_tick(&mut self, t: EnhTick) {
        let _span = crate::prof::acc::span("tick: weather and ambient");
        let extras = std::mem::replace(&mut self.fx_sim.next_extras, t.extras);
        self.fx_sim.extras = extras;
        self.fx_sim.cars = t.cars;
        self.fx_sim.stepped = Some(t.opts);
        let s = &mut self.fx_sim;
        let eye = t.camera.position;
        let o = t.opts;
        let interaction = o.wet_interaction;
        // The weather effects' view exists with a weather row on (as
        // `plan_screen_masks` sets `fx_view`).
        let scene = t.scene.filter(|_| o.weather_fx);
        // Wet Road Interaction's trail map: one commit a tick on a wet road.
        match scene {
            Some(sc) if interaction != 0 && sc.road_wet > 1.0e-3 => {
                let _span = crate::prof::acc::span("tick:   trail commit");
                s.trail.commit(interaction, eye, &sc, &s.cars, &s.extras)
            }
            _ => s.trail.reset(),
        }
        if let Some(sc) = scene
            && (o.rain_detail != 0 || o.spray || interaction != 0)
        {
            // The camera's velocity over the world clock (the streaks' motion).
            match s.last_eye {
                Some((c, p)) if sc.clock > c && sc.clock - c < 0.5 => {
                    let v = (eye - p) / (sc.clock - c);
                    if v.length() < 150.0 {
                        s.cam_vel = s.cam_vel.lerp(v, 0.5);
                    }
                    s.last_eye = Some((sc.clock, eye));
                }
                Some((c, _)) if c == sc.clock => {}
                _ => {
                    s.cam_vel = glam::Vec3::ZERO;
                    s.last_eye = Some((sc.clock, eye));
                }
            }
            s.spray.interaction = if interaction != 0 {
                s.cars
                    .iter()
                    .map(|c| {
                        wet_road::SprayCar::of(
                            c,
                            wet_road::match_extra(c, &s.extras),
                            sc.rain,
                            sc.road_wet,
                            sc.car_wet,
                        )
                    })
                    .collect()
            } else {
                Vec::new()
            };
            s.spray.tier = interaction;
            if o.spray || interaction != 0 {
                let _span = crate::prof::acc::span("tick:   spray step");
                // One wetness: the spray follows the film the rain left.
                s.spray.advance(&s.cars, sc.wetness().film, sc.clock);
            } else {
                s.spray = weather_fx::Spray::default();
            }
            s.spray.interaction.clear();
            // How much of the spray the eye sits in (the lens catches it).
            let hit = if interaction == 0 || s.spray.clouds() == 0 && s.spray.is_empty() {
                0.0
            } else {
                let _span = crate::prof::acc::span("tick:   spray hit");
                s.spray.hit(&s.cars, eye)
            };
            weather_fx::set_spray_hit(hit);
        }
        // Lens Drops (stepped by the world clock, with the eye's spray).
        if let Some(sc) = scene {
            let _span = crate::prof::acc::span("tick:   lens drops");
            weather_fx::lens_tick(sc.rain, sc.covered, sc.clock, t.speed, o.lens_drops, o.rain_detail >= crate::render::RenderOptions::RAIN_NEXT_GEN);
        }
        // Ambient Detail: the debris, grit and the volume's steps.
        if let (Some(a), Some(wet)) = (self.ambient.as_mut(), t.road_wet)
            && o.ambient_detail != 0
        {
            let _span = crate::prof::acc::span("tick:   ambient detail");
            a.tick(&ambient_detail::AmbientTick {
                tier: o.ambient_detail,
                cars: t.ambient,
                eye,
                time: t.time,
                wet,
            });
        }
        // The Next-Gen sky's panorama rows (one quarter a tick).
        if let Some(k) = t.sky.filter(|_| o.sky != 0) {
            self.sky_ticks.push(k);
            if self.sky_ticks.len() > 4 {
                self.sky_ticks.remove(0);
            }
        } else {
            self.sky_ticks.clear();
        }
    }

    /// A digest of the Enhanced effects' tick state here (the weather
    /// sim, the lens drops, Ambient Detail's CPU half, the sky's queued
    /// ticks): the rate tests compare it at equal ticks.
    pub fn enh_digest(&self) -> [u64; 4] {
        // (The sky's queued ticks are a frame's to commit, not tick state:
        // its rows are checked on the images.)
        let sky = 0;
        [
            self.fx_sim.digest(),
            weather_fx::lens_digest(),
            self.ambient.as_ref().map_or(0, |a| a.digest()),
            sky,
        ]
    }

    /// Weather effects: set up this frame's from the last tick's state (in
    /// game, a weather row on, the weather published); otherwise none
    /// drawn.
    pub(in crate::render) fn prepare_weather_fx(&mut self, gpu: &Gpu, opts: &RenderOptions) {
        let _span = crate::prof::acc::span("    weather fx prepare");
        let cars = self.fx_sim.cars.clone();
        let extras = self.fx_sim.extras.clone();
        // Wet Road Interaction: the trail map and the road's values.
        self.prepare_wet_road(gpu, &cars, &extras);
        let (Some(view), Some(scene)) = (self.fx_view, wet_car::scene()) else {
            self.fx_view = None;
            return;
        };
        // The spray as the ticks stepped it (Spray & Splashes, Wet Road
        // Interaction); the rain the frame's.
        let stepped = self.fx_sim.stepped_or(opts);
        let opts = &RenderOptions { wet_interaction: stepped.wet_interaction, spray: stepped.spray, ..*opts };
        let interaction = opts.wet_interaction;
        if opts.rain_detail == 0 && !opts.spray && interaction == 0 {
            self.fx_view = None;
            return;
        }
        let lamps = volumetric::nearest_lamps(&self.lamps, view.camera.position, view.camera.forward(), 32);
        let fx = self.weather_fx.get_or_insert_with(|| weather_fx::WeatherFx::new(gpu));
        fx.prepare(gpu, &view, &scene, &lamps, &cars, &self.fx_sim, opts);
        let spray = &self.fx_sim.spray;
        // The spray's volume puffs and mist through volume_fx, lit
        // by the lamps (64 nearest), the cars' lights and the underglow.
        if interaction == 0 || spray.clouds() == 0 && spray.is_empty() {
            if interaction == 0 {
                self.spray_volume = None;
            } else if let Some(v) = self.spray_volume.as_mut() {
                v.prepare(gpu, wet_spray::view_of(&view, scene.clock, Some(self.main_fog)), &Default::default(), &[]);
            }
            return;
        }
        let vp = crate::prof::acc::span("    spray volume particles");
        let particles = spray.volume_particles(view.camera.position, &cars);
        drop(vp);
        if crate::env_set!("NFSU2_WEATHER_DEBUG") {
            eprintln!(
                "wet spray: volume particles {} (clouds {}), hit {:.2}",
                particles.len(),
                spray.clouds(),
                weather_fx::spray_hit()
            );
        }
        let cam = &view.camera;
        if self.spray_volume.as_ref().is_none_or(|v| v.tier() != interaction) {
            self.spray_volume = Some(wet_spray::SprayVolume::new(gpu, interaction));
        }
        let lamps = volumetric::nearest_lamps(&self.lamps, cam.position, cam.forward(), volume_fx::MAX_LAMPS);
        let lights = wet_spray::scene_lights(
            &lamps,
            &cars,
            &extras,
            cam.position,
            modern_light::enabled(opts),
            wet_spray::neon_now(),
        );
        self.spray_volume.as_mut().unwrap().prepare(
            gpu,
            wet_spray::view_of(&view, scene.clock, Some(self.main_fog)),
            &lights,
            &particles,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The draw reads the rows the weather state was stepped with: the
    /// frame's before any tick, the last tick's after one, also when the
    /// frame's settings changed since.
    #[test]
    fn the_draw_reads_the_rows_the_state_was_stepped_with() {
        let frame = RenderOptions { wet_interaction: 2, spray: true, weather_fx: true, ..Default::default() };
        let mut sim = WeatherSim::default();
        assert_eq!(sim.stepped_or(&frame), EnhTickOptions::of_frame(&frame), "no tick: the frame's");
        sim.stepped = Some(EnhTickOptions::default());
        assert_eq!(sim.stepped_or(&frame), EnhTickOptions::default(), "the tick's, not the frame's newer rows");
    }
}
