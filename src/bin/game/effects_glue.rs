use crate::*;

impl Game {
    /// The particle effects while driving (frozen under a pause, as the
    /// game's 0x492E10 runs in the world update).
    /// `settings`: the tick's (`Pacing::tick_settings`, docs/frame-settings.md).
    pub(crate) fn particle_effects(&mut self, dt: f32, settings: &crate::sim::TickSettings) {
        let Some(d) = self.sim.drive.as_mut() else { return };
        let mut cars = vec![Some((&d.car, &d.state))];
        match &self.mode {
            Mode::Race(r) => cars.extend(r.racers.rivals.iter().map(|r| Some((&r.car, &r.state)))),
            // Keep parked slots as holes: carried particles and emitter caches
            // retain the same owner when earlier encounter slots activate.
            Mode::Explore => cars.extend(
                self.sim.encounters.iter().flat_map(|e| e.cars.iter()).map(|c| c.active.then_some((&*c.car, &c.state))),
            ),
            // A race's ending scene (EndingNis01..04) plays on under the
            // post-race menus: the world keeps updating, the particles with
            // it (0x492E10 runs in the world update 0x609CF0), so the scene
            // cars' burnout smokes (the original's EndingNis01 win under
            // Wine: tyre smoke behind the 240SX under Event Results,
            // docs/particles-smoke.md "Scene smoke"). Its cars: the player
            // and the AI winner it may bind (`ShopRun::companion`).
            _ if self
                .scenes.shop
                .as_ref()
                .is_some_and(|r| matches!(r.scene.kind, nfsu2::shop_scene::SceneKind::Ending(_))) =>
            {
                cars.extend(
                    self.scenes.shop.as_ref().and_then(|r| r.companion.as_ref()).map(|c| Some((&c.car, &c.state))),
                );
            }
            _ => {
                nfsu2::particles::drain();
                return;
            }
        }
        // Particle System (custom value 10, 0x870CEC).
        d.effects.system.particles_off = self.settings.custom[10] == 0;
        // The start scene's flag girl (0x436050 → 0x4335C0 → 0x433090).
        // prov: inferred game.particle_effects
        // **Inferred**: her flares fire before the world's particle update
        // (0x492E10) in the frame.
        if let (Mode::Race(_), Some(c)) = (&self.mode, self.scenes.crowd.as_ref()) {
            let flares = c.set.flares(c.frame / nfsu2::shop_scene::FPS, c.origin);
            d.effects.system.views = vec![d.camera.position];
            d.effects.flag_girl(&d.world.emitters, &flares);
        }
        // The race start scene's car effects (`shop_scene::effects`): the
        // player on its grid slot's car, each rival on its own (the
        // prov: inferred game.particle_effects-2
        // clone's Car<n> = grid slot n; **inferred**, 0x827BA8's slot → car
        // binding is not traced).
        let scene_flames: Vec<f32> = match (&self.mode, self.scenes.shop.as_ref()) {
            (Mode::Race(r), Some(run)) if !run.scene.effect_events.is_empty() => {
                let at = |slot: usize| {
                    nfsu2::shop_scene::effects::intensity(
                        &run.scene.effect_events,
                        slot,
                        nfsu2::shop_scene::effects::NITRO,
                        run.frame,
                    )
                };
                std::iter::once(at(r.grid_slot + 1)).chain(r.racers.rivals.iter().map(|rv| at(rv.slot + 1))).collect()
            }
            _ => Vec::new(),
        };
        let order = d.effects.order;
        d.effects.rain = d.weather.amount;
        d.effects.step_in_order(&d.world, d.camera.position, &cars, &scene_flames, order, dt, settings);
    }
}
