use crate::*;

mod markers;
pub(crate) use markers::*;

/// The particle effects ([`nfsu2::particles`]): the contact bursts the
/// physics posts, the cars' ground effects (tyre smoke, spray, bottom-out)
/// and the free-roam marker effects.
pub(crate) struct Effects {
    pub(crate) system: nfsu2::particles::System,
    pub(crate) rng: nfsu2::particles::Rng,
    /// The surface table's emitter lists the ground effects fire (0x5E0630,
    /// the object at 0x883E68: one list per (event, surface), the named
    /// group's templates cloned, each with its own axes), made when first
    /// fired; shared by every car, as natively.
    pub(crate) ground: std::collections::HashMap<(usize, u8), Vec<(glam::Mat4, nfsu2::particles::Emitter)>>,
    /// Per car: the ground effects' skid timers (0x5F5740) and the query
    /// for the surface under the car.
    pub(crate) ground_cars: std::collections::HashMap<usize, (nfsu2::particles::GroundFx, nfsu2::ground::GroundQuery)>,
    /// The nitrous flames' emitters, per (car, description, exhaust).
    pub(crate) flames: std::collections::HashMap<(usize, usize, usize), nfsu2::particles::Emitter>,
    /// The twelve marker effects (0x890130, 0x30 bytes each; see
    /// [`MARKER_EFFECTS`]): where each stands (+0x10) and its emitters
    /// (+0x28), or off.
    pub(crate) markers: [Option<(Vec3, Vec<nfsu2::particles::Emitter>)>; 12],
    /// The track's placed emitters (chunk 0x35020), started once the
    /// world is known (0x491C70 → 0x48BC50), with the switch each needs.
    pub(crate) placed: Option<Vec<(u32, nfsu2::particles::Emitter)>>,
    /// The flag girl's flare emitters (the props object 0x439780 makes
    /// once, [0x827B28]: one per description of the group
    /// "XECH_FLAG_GIRL_FLARE", 0x48B8B0 — glow, smoke, sparks in GLOBALB).
    pub(crate) flag_girl: Vec<nfsu2::particles::Emitter>,
    /// The cars' purge smoke emitters (kinds 13 / 14), per car.
    pub(crate) purges: std::collections::HashMap<usize, PurgeSmoke>,
    /// The turbo's flame pops (0x5EBE10 at a shift, 0x5F5F5F revving in
    /// neutral), per car: what each remembers between updates.
    pub(crate) pops: std::collections::HashMap<usize, nfsu2::carfx_pops::Pops>,
    /// Seconds of world updates (the pops' 0.5 s gate reads the game's
    /// clock 0x865188 for this).
    pub(crate) clock: f32,
    /// Particles High / Ultra: the native smoke's births, detailed
    /// on the GPU (`render::gpu_particles`). Original: unused.
    pub(crate) detail: nfsu2::render::gpu_particles::Detail,
    /// Particles Cinematic (docs/notes/graphics/smoke.md "Particles Cinematic: the simulation and the billboard drawing"): the native
    /// smoke's births as billows and haze at a reduced resolution.
    pub(crate) cinematic: nfsu2::render::smoke_cinematic::Cinematic,
    /// Animated Water (docs/notes/graphics/water.md): the water's mesh and GPU half.
    pub(crate) water: nfsu2::render::water::Water,
    /// Sparks & Fire (docs/notes/graphics/sparks-fire.md): the native spark births as
    /// thrown fragments, the flames and their lights. Original: unused.
    pub(crate) sparks_fire: nfsu2::render::sparks_fire::SparksFire,
    /// What the owner hands [`Effects::step`] and the window's draw:
    /// [`EffectsOrder::NATIVE`] but in the nitro comparison tests.
    pub(crate) order: EffectsOrder,
    /// The rain's amount at this tick (weather +0x1F0), set by the
    /// tick before the step: Cinematic smooths its effect on new smoke per
    /// tick (`Feeder::wet`).
    pub(crate) rain: f32,
}

/// A car's purge smoke (car effect kinds 13 and 14): one emitter per
/// cloned template of each kind's group (0x48B8B0), recoloured once into
/// a colour slot (0x48B920).
#[derive(Default)]
pub(crate) struct PurgeSmoke {
    pub(crate) emitters: [Vec<nfsu2::particles::Emitter>; 2],
    /// The groups the emitters were made for (a part change remakes them).
    pub(crate) groups: [u32; 2],
}

impl PurgeSmoke {
    /// 0x615CD0(13, 1.0, …), which runs kind 14 after 13: each kind's
    /// emitters, at each point of their marker group (description +0x5C:
    /// 28 the hood markers, 29 the wheel points), get the car's matrix ×
    /// (their own axes with the point's mirror flags applied, at the
    /// point) and the car's velocity, then burst −1.0 (0x491E50 with the
    /// owner key).
    pub(crate) fn burst(
        &mut self,
        system: &mut nfsu2::particles::System,
        library: &nfsu2::particles::Library,
        fx: &nfsu2::car::PurgeFx,
        body: glam::Mat4,
        velocity: Vec3,
        key: u32,
        rng: &mut nfsu2::particles::Rng,
    ) {
        if self.groups != fx.groups {
            self.groups = fx.groups;
            for k in 0..2 {
                let [hue, sat, lum] = fx.hsl[k];
                self.emitters[k] = library
                    .group_templates(fx.groups[k])
                    .into_iter()
                    .map(|(d, m)| {
                        let mut e = nfsu2::particles::Emitter::new(d, m, (rng.next() * 4.0e9) as u32);
                        e.palette = system.palette(nfsu2::particles::recolor(&library.descriptions[d], hue, sat, lum));
                        e
                    })
                    .collect();
            }
        }
        for k in 0..2 {
            for e in &mut self.emitters[k] {
                let Some(d) = library.descriptions.get(e.description) else { continue };
                // The template's own matrix (the clone's emitter keeps it
                // in `start`; only the axes are used).
                let own = library
                    .templates
                    .iter()
                    .find(|t| library.by_hash(t.0) == Some(e.description))
                    .map_or(glam::Mat4::IDENTITY, |t| t.1);
                let points: &[(Vec3, [bool; 3])] = match d.marker_group {
                    28 => &fx.markers[0],
                    29 => &fx.markers[1],
                    _ => &[],
                };
                for &(at, mirror) in points {
                    let flip = Vec3::new(
                        if mirror[0] { -1.0 } else { 1.0 },
                        if mirror[1] { -1.0 } else { 1.0 },
                        if mirror[2] { -1.0 } else { 1.0 },
                    );
                    let axes = glam::Mat4::from_cols(
                        (own.x_axis.truncate() * flip).extend(0.0),
                        (own.y_axis.truncate() * flip).extend(0.0),
                        (own.z_axis.truncate() * flip).extend(0.0),
                        at.extend(1.0),
                    );
                    e.transform = body * axes;
                    e.velocity = velocity;
                    e.key = key;
                    system.emit(library, e, -1.0);
                }
            }
        }
    }
}

impl Default for Effects {
    fn default() -> Effects {
        // 0x7F3940's seed is the EXE's data (**inferred**: any non-zero).
        Effects {
            system: Default::default(),
            rng: nfsu2::particles::Rng(0x12345678),
            ground: Default::default(),
            ground_cars: Default::default(),
            flames: Default::default(),
            markers: Default::default(),
            placed: None,
            flag_girl: Vec::new(),
            purges: Default::default(),
            pops: Default::default(),
            clock: 0.0,
            detail: Default::default(),
            cinematic: Default::default(),
            water: Default::default(),
            sparks_fire: Default::default(),
            order: EffectsOrder::NATIVE,
            rain: 0.0,
        }
    }
}

impl Effects {
    /// 0x61FB10(10, `amount`, …) → 0x615CD0 on one car: each CARFX_NITRO
    /// emitter bursts −`amount` at every exhaust marker.
    pub(crate) fn flame_burst(
        &mut self,
        library: &nfsu2::particles::Library,
        ci: usize,
        car: &Car,
        state: &CarState,
        amount: f32,
    ) {
        use nfsu2::particles as fx;
        let body = glam::Mat4::from_rotation_translation(state.orientation, state.position);
        // 0x615CD0 (**verified** 2026-11-04, REA): each of the kind's
        // emitters (the group's templates cloned by 0x48B8B0, render
        // +0x458 + 8·kind) keeps its own axes (the template's matrix);
        // only the translation becomes the marker's position (entry
        // +0x10 → marker +0x40..+0x48) and its axes are mirrored by the
        // entry's flag bits, which 0x63986C..0x6398AF clear for the
        // exhaust list. The list is render +0x548 + 8 · the
        // description's marker group (+0x5C): XES_NOS_FlameA's is 10,
        // render +0x598, the exhaust markers 0x639843 files. The
        // marker's own axes play no part (the clone took them before:
        // a marker turned against the template sent the flame into
        // the bodywork).
        for (d, own) in library.group_templates(nfsu2::formats::hash::bin_hash("CARFX_NITRO")) {
            for (mi, &m) in car.exhausts.iter().enumerate() {
                let transform = body
                    * glam::Mat4::from_cols(
                        own.x_axis.truncate().extend(0.0),
                        own.y_axis.truncate().extend(0.0),
                        own.z_axis.truncate().extend(0.0),
                        m.w_axis,
                    );
                let seed = (self.rng.next() * 4.0e9) as u32;
                let e = self.flames.entry((ci, d, mi)).or_insert_with(|| fx::Emitter::new(d, transform, seed));
                e.transform = transform;
                e.velocity = state.velocity;
                // The owner key (0x491E50's third argument, the render
                // object's first dword): carried particles follow it.
                e.key = FLAME_KEY + ci as u32;
                self.system.emit(library, e, -amount);
            }
        }
    }

    /// One frame: fire the posted bursts, run the sliding wheels'
    /// emitters, age and move every particle.
    /// `scene_flames`: per car (as `cars`), the race start scene's
    /// CARFX_NITRO intensity this update (`shop_scene::effects`; 0 none).
    /// The game steps through [`Effects::step_in_order`] (the drive's
    /// `order`); this native-order form is the harness's and the tests'.
    #[cfg(any(test, feature = "dev-harness"))]
    pub(crate) fn step(
        &mut self,
        world: &nfsu2::world::World,
        view: Vec3,
        cars: &[Option<(&Car, &CarState)>],
        scene_flames: &[f32],
        dt: f32,
        settings: &crate::sim::TickSettings,
    ) {
        self.step_in_order(world, view, cars, scene_flames, EffectsOrder::NATIVE, dt, settings);
    }

    /// [`Effects::step`] in an explicit `order`: where the car effects
    /// fire against the particle update ([`EffectsOrder::NATIVE`]
    /// everywhere but the tests that compare the behaviour before the
    /// fixes).
    pub(crate) fn step_in_order(
        &mut self,
        world: &nfsu2::world::World,
        view: Vec3,
        cars: &[Option<(&Car, &CarState)>],
        scene_flames: &[f32],
        order: EffectsOrder,
        dt: f32,
        settings: &crate::sim::TickSettings,
    ) {
        let library = &world.emitters;
        self.clock += dt;
        self.start_placed(world);
        // Record this update's births for the detailed smoke (no
        // change to the spawns themselves; None on Original).
        let particle_tier = settings.particles;
        let cinematic = nfsu2::render::smoke_cinematic::active(particle_tier);
        // Cinematic records into its own feeder (same births).
        let detail = if cinematic {
            Some(nfsu2::render::smoke_cinematic::feed_params())
        } else {
            nfsu2::render::gpu_particles::tier_params(particle_tier)
        };
        // Sparks & Fire records the same births (its spark kinds only).
        let sparks_fire = settings.sparks_fire != 0;
        self.system.births = (detail.is_some() || sparks_fire).then(Vec::new);
        self.system.views = vec![view];
        let contacts = self.contact_bursts(library, sparks_fire);
        self.run_emitters(world, dt);
        let velocities: Vec<Option<Vec3>> = cars.iter().map(|c| c.map(|c| c.1.velocity)).collect();
        let emit_first = order.emit_before_update;
        // The world update 0x609CF0 runs the particle update (0x492E10 at
        // 0x60A191) *before* each car's update 0x5F5740 (0x60A205), which
        // fires the car effects (**verified** 2026-11-04, disassembly): a
        // flame is drawn where it was emitted, at the exhaust. Emitting
        // first and then moving it by the car's velocity (the clone until
        // then) put each new flame a frame's travel ahead of the exhaust —
        // 0.8 m at 180 km/h, inside the car's body, so the flame vanished
        // at speed.
        if emit_first {
            self.car_effects(world, cars, scene_flames, dt);
        }
        self.system.update(library, dt, |key| {
            key.checked_sub(FLAME_KEY).and_then(|i| velocities.get(i as usize).copied().flatten())
        });
        if !emit_first {
            self.car_effects(world, cars, scene_flames, dt);
        }
        self.feed_births(world, cars, &contacts, detail, cinematic, sparks_fire, dt);
    }

    /// The placed emitters: bound to their descriptions by name hash
    /// (0x48BC00) and started in their stored state (0x48BC50; states
    /// other than 0 and 1 never run), once.
    fn start_placed(&mut self, world: &nfsu2::world::World) {
        let library = &world.emitters;
        if self.placed.is_none() {
            let mut placed = Vec::new();
            for p in world.placed_emitters.iter().filter(|p| p.state <= 1) {
                let Some(d) = library.by_hash(p.description) else { continue };
                let seed = (self.rng.next() * 4.0e9) as u32;
                let mut e = nfsu2::particles::Emitter::new(d, p.transform, seed);
                e.on = p.state == 1;
                e.start(library);
                placed.push((p.switch, e));
            }
            self.placed = Some(placed);
        }
    }

    /// The posted contact bursts (drained), with each burst's births and
    /// push for the Sparks & Fire tier's intensity (nothing read on
    /// Original).
    fn contact_bursts(
        &mut self,
        library: &nfsu2::particles::Library,
        sparks_fire: bool,
    ) -> Vec<nfsu2::render::sparks_fire::Contact> {
        use nfsu2::particles as fx;
        let mut contacts = Vec::new();
        for b in fx::drain() {
            let first = self.system.births.as_ref().map_or(0, |v| v.len());
            fx::burst(&mut self.system, library, &b, &mut self.rng);
            let end = self.system.births.as_ref().map_or(0, |v| v.len());
            if sparks_fire && end > first {
                let two_sided = b.group == nfsu2::formats::hash::bin_hash("XECI_CAR");
                contacts.push(nfsu2::render::sparks_fire::Contact {
                    births: first..end,
                    slide: b.slide,
                    impulse: b.impulse,
                    normal: b.normal,
                    two_sided,
                });
                if debug_env().fx_contacts {
                    eprintln!(
                        "contact group {:08X} share {:.2} slide {:.1} impulse {:.1} births {} intensity {:.2}",
                        b.group,
                        b.share,
                        b.slide,
                        b.impulse,
                        end - first,
                        nfsu2::render::sparks_fire::intensity(b.slide, b.impulse)
                    );
                }
            }
        }
        contacts
    }

    /// One car's ground effects (0x5F5740's first part, each car's update
    /// after the particle update): [`nfsu2::particles::GroundFx`]'s fires,
    /// each bursting the surface event's list at its markers.
    fn ground_effects(&mut self, world: &nfsu2::world::World, ci: usize, car: &Car, state: &CarState, dt: f32) {
        use nfsu2::particles as fx;
        let library = &world.emitters;
        let rot = glam::Mat3::from_quat(state.orientation);
        let com = state.com_world(Vec3::from(car.car_type.handling.centre_of_mass));
        let (ground, query) = self.ground_cars.entry(ci).or_default();
        let centre_surface = world.ground_for(query, com).map(|g| g.2);
        let input = fx::CarIn {
            wheels: std::array::from_fn(|i| {
                let w = &state.wheels[i];
                fx::WheelIn {
                    on_ground: w.on_ground,
                    surface: w.surface,
                    slip: w.exported_slip,
                    lateral: w.lateral_slip,
                    spin: w.spin,
                }
            }),
            speed: state.velocity.length(),
            bump: state.bump_totals.iter().sum(),
            centre_surface,
            distance: self.system.views.first().map_or(0.0, |v| v.distance(com)),
        };
        let fires = ground.update(&input, dt);
        if fires.is_empty() {
            return;
        }
        let wheels: [Vec3; 4] = std::array::from_fn(|i| car.car_type.wheels[i].position);
        for f in fires {
            let event = f.event();
            let list = self.ground.entry((event, f.surface)).or_insert_with(|| {
                let Some(name) = fx::surface_events(f.surface)[event].0 else { return Vec::new() };
                library
                    .group_templates(nfsu2::formats::hash::bin_hash(name))
                    .into_iter()
                    .map(|(d, own)| (own, fx::Emitter::new(d, own, (self.rng.next() * 4.0e9) as u32)))
                    .collect()
            });
            // 0x61FB10: the wheel kinds stand on the centre of mass (+0x60,
            // the body's matrix +0x90), the others on the origin (+0x50,
            // +0xD0); both are the body's rotation here.
            let base = if f.wheel.is_some() { com } else { state.position };
            let velocity = match f.velocity {
                fx::FireVelocity::Car => state.velocity,
                fx::FireVelocity::Body(v) => rot * v,
                // **Inferred**: the wheel's matrix (+0x130 + 0x40·i) as the
                // body's rotation (the rear wheels, the only ones spinning
                // smoke, do not steer).
                fx::FireVelocity::WheelPlusCar(v) => rot * v + state.velocity,
            };
            for (own, e) in list.iter_mut() {
                let Some(d) = library.descriptions.get(e.description) else { continue };
                let group = f.wheel.map_or(d.marker_group, |w| 5 + w as u32);
                let Some(at) = fx::marker(group, &wheels) else { continue };
                // car × (the template's own axes at the marker), 0x615CD0.
                e.transform = glam::Mat4::from_rotation_translation(state.orientation, base)
                    * glam::Mat4::from_cols(
                        own.x_axis.truncate().extend(0.0),
                        own.y_axis.truncate().extend(0.0),
                        own.z_axis.truncate().extend(0.0),
                        at.extend(1.0),
                    );
                e.velocity = velocity;
                // The owner key (the render object's first dword): the
                // carried ones (deep water's drops) follow the car.
                e.key = FLAME_KEY + ci as u32;
                self.system.emit(library, e, -f.intensity);
            }
        }
    }

    /// The placed and the marker effects' emitters (0x492E10).
    fn run_emitters(&mut self, world: &nfsu2::world::World, dt: f32) {
        let library = &world.emitters;
        // 0x492E10's first loop: a placed emitter runs while its switch is
        // 0 or held (0x5D7450 on the list 0x88A28C).
        for (switch, e) in self.placed.iter_mut().flatten() {
            if *switch == 0 || world.switches.contains(switch) {
                self.system.run(library, e, dt);
            }
        }
        // The marker effects' emitters run with the rest (0x492E10).
        for (_, emitters) in self.markers.iter_mut().flatten() {
            for e in emitters {
                self.system.run(library, e, dt);
            }
        }
    }

    /// The cars' own effects (each car's update 0x5F5740): the turbo's
    /// pops, the start scene's flames, the purge and the nitrous flame.
    fn car_effects(
        &mut self,
        world: &nfsu2::world::World,
        cars: &[Option<(&Car, &CarState)>],
        scene_flames: &[f32],
        dt: f32,
    ) {
        let library = &world.emitters;
        let this = self;
        // The nitrous flame (0x5F5F0B, each car's update): while a shot
        // runs (0x59FA40) and 0x5ECDC0 gives 0, 0x61FB10(10, 1.0, 0, 0)
        // fires car effect kind 10 — `CARFX_NITRO` by the name map 0x430C90
        // (11 EXHAUST_SMOKE, 12 EXHAUST_BLOWOFF, 13 NOS_BLOWOFF, the
        // purge's kind, 0x5F5FF9). 0x615CD0 then bursts each of the kind's
        // emitters (render +0x458 + 8·kind) at every marker of its group
        // (render +0x548 + 8·group) with −1.0 (0x491E50), the emitter at
        // car × (its own axes at the marker) and +0x80 the car's velocity.
        // prov: inferred game.step
        // **Inferred**: the kind-10 emitter list holds the `CARFX_NITRO`
        // group's templates (the name map gives the kind's group; the
        // list's fill from the car data is not traced). The purge smoke
        // (kind 13 / 14): `PurgeSmoke`.
        for (ci, (car, state)) in cars.iter().enumerate().filter_map(|(i, c)| c.map(|c| (i, c))) {
            // The ground effects come first in the car's update (0x5F57D0..0x5F5DBB).
            this.ground_effects(world, ci, car, state, dt);
            // The turbo's flame pops (CARFX_NITRO, intensity L · 2/7):
            // at a shift (Car_Frame 0x6012C0 → 0x5EBE10) and revving in
            // neutral (0x5F5F5F, after the shot's flame or purge in
            // 0x5F5740). See `carfx_pops`.
            let car_in = nfsu2::carfx_pops::CarIn::of(car, state);
            let clock = this.clock;
            let pop = this.pops.entry(ci).or_default();
            let mut pops = Vec::new();
            pops.extend(pop.rev(&car_in));
            pops.extend(pop.shift(&car_in, clock));
            // The player's car (first in `cars`) sends message 9 after
            // a pop (0x5EBF0E / 0x5F6059): CAR_Sputter's p8.
            if ci == 0 && !pops.is_empty() {
                nfsu2::audio::carfx::note_backfire();
            }
            for amount in pops {
                this.flame_burst(library, ci, car, state, amount);
            }
            // The scene's car effects (0x43966D..0x439772, every scene
            // update): CARFX_NITRO with its intensity above 0 fires
            // 0x61FB10(10, intensity, parameter, 0), the burst −intensity
            // (SEQ_CAR_NITRO: 2 for three scene frames).
            if let Some(&amount) = scene_flames.get(ci).filter(|&&a| a > 0.0) {
                this.flame_burst(library, ci, car, state, amount);
            }
            let stage = state.drivetrain.as_ref().map_or(0, |t| t.nitrous);
            // The purge (0x5F5FF9: kind 13 when 0x5ECDC0 is not 0, which
            // runs 14 too): the purge part's smoke from the hood markers
            // and the front wheels, the car's velocity carried.
            if stage != 0 && nfsu2::audio::carfx::purging(stage, state.speed(), state.gas()) {
                if let Some(fx) = car.purge.as_ref() {
                    let body = glam::Mat4::from_rotation_translation(state.orientation, state.position);
                    this.purges.entry(ci).or_default().burst(
                        &mut this.system,
                        library,
                        fx,
                        body,
                        state.velocity,
                        FLAME_KEY + ci as u32,
                        &mut this.rng,
                    );
                }
                continue;
            }
            if stage == 0 {
                continue;
            }
            this.flame_burst(library, ci, car, state, 1.0);
        }
    }

    /// This update's births to the enhanced tiers: Sparks & Fire,
    /// the detailed smoke or Cinematic. Nothing on Original.
    #[allow(clippy::too_many_arguments)]
    fn feed_births(
        &mut self,
        world: &nfsu2::world::World,
        cars: &[Option<(&Car, &CarState)>],
        contacts: &[nfsu2::render::sparks_fire::Contact],
        detail: Option<nfsu2::render::gpu_particles::TierParams>,
        cinematic: bool,
        sparks_fire: bool,
        dt: f32,
    ) {
        let library = &world.emitters;
        let births = self.system.births.take();
        if sparks_fire {
            let ground = |p: Vec3| world.road_height_below(p + Vec3::Z * 1.0);
            self.sparks_fire.sim.record(library, births.as_deref().unwrap_or(&[]), contacts, &ground);
            self.sparks_fire.sim.step(dt);
        } else if !self.sparks_fire.sim.sparks.is_empty() {
            self.sparks_fire.sim.clear();
        }
        if let (Some(tier), Some(births)) = (detail, births) {
            let cars = cars.iter().map(|c| c.map(|(_, s)| (s.position, s.velocity))).collect();
            if cinematic {
                self.cinematic.feeder.record(library, births, cars, dt, tier, self.rain);
            } else {
                self.detail.feeder.record(library, births, cars, dt, tier, self.rain);
            }
        }
        // The Cinematic path off: its rain smoothing starts over next time.
        if !cinematic {
            self.cinematic.feeder.wet = None;
        }
    }
}

/// Where the cars' effects fire against the particle update, and how the
/// window draws the cars' carried particles. The game always runs
/// [`EffectsOrder::NATIVE`]; [`EffectsOrder::BEFORE_FIXES`] is the
/// behaviour before the 2026-11-04 fixes, which the nitro tests compare
/// against (it replaced the tests' thread-local switches).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct EffectsOrder {
    /// The car effects emitted before the particle update (the native
    /// order is after it, 0x609CF0).
    pub(crate) emit_before_update: bool,
    /// The carried particles drawn where emitted instead of with their
    /// owner's blend (`draw_between`).
    pub(crate) unblended_particles: bool,
}

impl EffectsOrder {
    pub(crate) const NATIVE: EffectsOrder = EffectsOrder { emit_before_update: false, unblended_particles: false };
    #[cfg(test)]
    pub(crate) const BEFORE_FIXES: EffectsOrder = EffectsOrder { emit_before_update: true, unblended_particles: true };
}

/// The clone's owner keys for the cars' carried particles.
pub(crate) const FLAME_KEY: u32 = 0x4E4F_0000;
