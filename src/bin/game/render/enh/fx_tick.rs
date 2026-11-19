//! Enhanced effects on ticks (docs/original/uncapped-fps.md, "Enhanced effects on ticks"): the
//! Enhanced effects that simulate or accumulate step once per tick, here,
//! from the tick's own state (the cars' poses, the camera, the weather and
//! the clocks as the tick left them), never from a drawn frame:
//!
//! - Wet Car's per-car state (`CarRenderer::tick_wet`; its last two ticks'
//!   values kept for the draw's blend);
//! - the weather effects' CPU half (`WorldRenderer::enh_tick`): the spray
//!   pool and the camera's velocity, the eye's share of the spray, the lens
//!   drops, Wet Road Interaction's trail-map commit;
//! - Ambient Detail's debris, grit and volume steps;
//! - the Next-Gen sky's panorama rows (one quarter a tick).
//!
//! The lists the draw's glow pass pushed before (the cars' lights and
//! wheels for the wet road and the spray, the cars for Ambient Detail) are
//! built here with the same cars, in the same order, from the tick state.
//! Smoke (High / Ultra / Cinematic) and Sparks & Fire record and step in
//! the world update itself (`Effects::step_in_order`).
use crate::*;

impl Game {
    /// The tick hook (the end of `Game::update`): the Enhanced
    /// effects' simulations stepped once with this tick's state. Nothing
    /// without a drive or a world renderer (nothing is drawn then either).
    pub(crate) fn enh_fx_tick(&mut self, dt: f32) {
        let Some(d) = self.sim.drive.as_ref() else { return };
        if self.render.world.is_none() {
            return;
        }
        let _span = nfsu2::prof::acc::span("tick: enhanced hook");
        // The tick's index from boot (`GameClock::tick`, the same tick at
        // every display rate).
        let tick = self.pacing.clocks.tick;
        let time = self.seconds();
        // The tick's settings (`Pacing::tick_settings`).
        let ts = self.pacing.tick_settings;
        // The weather the cars and the spray read (`wet_car::publish_weather`'s).
        let scene = ts.weather_fx.then_some(nfsu2::render::wet_car::Scene {
            rain: d.weather.amount,
            road_wet: d.weather.road_wet,
            car_wet: d.weather.car_wet,
            covered: d.weather.under_cover,
            clock: d.clock,
        });
        // Wet Car: each car the cars pass draws, in its order (the player
        // unless the view hides it, the companion, the rivals, the active
        // encounter racers, the traffic in a lane), stepped with its pose.
        let race = match &self.mode {
            Mode::Race(r) => Some(r),
            _ => self.sim.paused_race.as_ref(),
        };
        let mut drawn: Vec<(&DrawId, &CarState)> = Vec::new();
        let hidden = native_mover_type(&self.mode, Some(d)) == 1 && d.chase.hides_car() && self.photo.is_none();
        if !hidden {
            drawn.push((&d.car_id, &d.state));
        }
        if let Some(c) = self.scenes.shop.as_ref().and_then(|run| run.companion.as_ref()) {
            drawn.push((&c.id, &c.state));
        }
        if let Some(r) = race {
            drawn.extend(r.racers.rivals.iter().map(|rv| (&rv.id, &rv.state)));
        }
        if let (Mode::Explore, Some(e)) = (&self.mode, self.sim.encounters.as_ref()) {
            drawn.extend(e.cars.iter().filter(|c| c.active).map(|c| (&c.id, &c.state)));
        }
        let traffic = match &self.mode {
            Mode::Explore => self.sim.traffic.as_ref(),
            Mode::Race(r) => r.racers.traffic.as_ref().filter(|t| !t.paused),
            _ => None,
        };
        if let Some(t) = traffic {
            drawn.extend(t.cars.iter().filter(|c| c.lane.is_some() && c.active).map(|c| (&c.id, &c.state)));
        }
        let wet_span = nfsu2::prof::acc::span("tick: wet car");
        let mut cars = Vec::new();
        for (id, state) in drawn {
            if let Some(cr) = self.render.cars.get_mut(id)
                && let Some(sample) = cr.tick_wet(scene.as_ref(), state.transform())
            {
                nfsu2::render::weather_fx::add_car(&mut cars, sample);
            }
        }
        drop(wet_span);
        // The glow pass's lists (its cars: the player, the race's rivals,
        // the active encounter racers in free roam, the traffic in a lane
        // and not faded out), when that pass runs its car hook.
        let mut extras = Vec::new();
        let mut ambient = Vec::new();
        let glows = ts.switches.light_glow || ts.wet_interaction != 0;
        if glows {
            let mut car = |car: &Car, state: &CarState, holds: bool| {
                let t = state.transform();
                nfsu2::render::wet_road::add_extra(
                    &mut extras,
                    nfsu2::render::wet_road::car_extra(
                        t,
                        &car.light_flares,
                        car.headlight_pool_color,
                        state.brake_lights(holds),
                        state.wheels.map(|w| (w.slip, w.lateral_slip, w.on_ground)),
                        ts.wet_interaction,
                    ),
                );
                nfsu2::render::ambient_detail::add_car(
                    &mut ambient,
                    nfsu2::render::ambient_detail::car_in(
                        t,
                        state.velocity,
                        car.car_type.wheels.each_ref().map(|w| (w.position, w.radius)),
                        state.wheels.map(|w| (w.slip.abs().max(w.lateral_slip.abs()), w.on_ground)),
                        car.headlight_pool_color,
                        state.brake_lights(holds),
                    ),
                    ts.ambient_detail,
                );
            };
            car(&d.car, &d.state, false);
            for r in race.iter().flat_map(|r| &r.racers.rivals) {
                car(&r.car, &r.state, true);
            }
            if let (Mode::Explore, Some(e)) = (&self.mode, self.sim.encounters.as_ref()) {
                for c in e.cars.iter().filter(|c| c.active) {
                    car(&c.car, &c.state, true);
                }
            }
            let ring = ts.pop_free && ts.lod_fade;
            for c in traffic.iter().flat_map(|t| t.cars.iter().filter(|c| crate::render::frame::traffic_drawn(c))) {
                let share = if ring {
                    nfsu2::ai::traffic::ring_presence(c.state.position.distance(d.state.position))
                } else {
                    1.0
                };
                if share > 0.0 {
                    car(&c.car, &c.state, false);
                }
            }
        }
        let sky = d.time_of_day.sky();
        let t = nfsu2::render::weather_fx::EnhTick {
            tick,
            dt,
            camera: d.camera,
            scene,
            road_wet: Some(d.weather.road_wet),
            speed: d.state.velocity.length(),
            time,
            cars,
            extras,
            ambient,
            sky: Some(nfsu2::render::sky_ng::SkyTick {
                tick,
                eye: d.camera.position,
                time,
                rain: d.weather.amount,
                set: sky.set,
                morph: sky.morph,
            }),
            opts: ts.enh_options(),
        };
        if let Some(r) = self.render.world.as_mut() {
            r.enh_tick(t);
        }
        enh_hash_line(self, tick);
    }

    /// The Enhanced effects' tick state as digests (the rate tests and
    /// `NFSU2_ENH_HASH` compare them at equal ticks across display rates):
    /// the weather sim (spray, camera velocity, trail map), the lens drops,
    /// Ambient Detail, the sky's queued ticks, Wet Car (every car's last
    /// tick), Sparks & Fire, the smoke feeders' clocks and rain.
    #[cfg(any(test, feature = "dev-harness"))]
    pub(crate) fn enh_digest(&self) -> [u64; 8] {
        let mut out = [0u64; 8];
        if let Some(r) = self.render.world.as_ref() {
            out[..4].copy_from_slice(&r.enh_digest());
        }
        let mut wet = 0u64;
        self.drawn_car_states(|k, _| {
            if let Some(cr) = self.render.cars.get_key(k) {
                wet = (wet ^ cr.wet_digest()).wrapping_mul(0x100_0000_01b3);
            }
        });
        out[4] = wet;
        if let Some(d) = self.sim.drive.as_ref() {
            let e = &d.effects;
            out[5] = e.sparks_fire.sim.digest();
            let f = |f: &nfsu2::render::gpu_particles::Feeder| {
                f.time.to_bits() ^ u64::from(f.wet.unwrap_or(-1.0).to_bits()).rotate_left(29)
            };
            out[6] = f(&e.cinematic.feeder);
            out[7] = f(&e.detail.feeder);
        }
        out
    }
}

/// `NFSU2_ENH_HASH=file` (dev-harness): one line per tick, the tick and
/// [`Game::enh_digest`]'s parts.
#[cfg(feature = "dev-harness")]
fn enh_hash_line(game: &Game, tick: u64) {
    use std::io::Write as _;
    static SINK: std::sync::OnceLock<Option<std::sync::Mutex<std::fs::File>>> = std::sync::OnceLock::new();
    let sink = SINK.get_or_init(|| {
        std::env::var_os("NFSU2_ENH_HASH").and_then(|p| std::fs::File::create(p).ok()).map(std::sync::Mutex::new)
    });
    if let Some(f) = sink {
        let d = game.enh_digest();
        let mut line = format!("{tick}");
        for v in d {
            line.push_str(&format!("\t{v:016x}"));
        }
        let _ = writeln!(f.lock().unwrap(), "{line}");
    }
}

#[cfg(not(feature = "dev-harness"))]
fn enh_hash_line(_game: &Game, _tick: u64) {}
