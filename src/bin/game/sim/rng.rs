//! The game's random-number routines on the shared generator (0x7EA624,
//! `Game::rng`) and the world generator (world +0xF8) in the original's
//! sites and order (docs/original/rng.md). The draws whose values the
//! clone does not read are in `nfsu2::rng_sites`.

use crate::*;

impl Game {
    /// The start-up's draws after the boot seed (0x57ED20's chain): the two
    /// views' lens set-up (0x57ED7C → 0x5C2FA0, 120 draws; the clone keeps
    /// view 1's), the input binding's draw (0x57ED90 → 0x579560, 1), then
    /// the traffic manager list's first build (0x4FE9F0 → 0x41AAF0 →
    /// 0x412840 with no world: 15 × 2). **Measured** on the pinned original:
    /// 151 shared draws from the process start to the main menu with the
    /// sound off (c1/d1 `boot`); the sequence log gives this order, then
    /// with the sound on the speech library's start (0x4866F0) and EA
    /// Trax's first pick, then the main menu's car render info (0x6383A5,
    /// [`Game::menu_car_draw`]). Made again when the window reseeds the
    /// generator from the clock.
    pub(crate) fn boot_draws(&mut self) {
        self.sim.traffic_paints = nfsu2::rng_sites::traffic_paints(&self.game_data.parts, &self.game_data.car_types);
        self.sim.lens = nfsu2::weather::LensSetup::boot(&mut self.rng);
        nfsu2::rng_sites::input_bind(&mut self.rng);
        nfsu2::rng_sites::model_slots(&mut self.rng, None, &self.sim.traffic_paints);
        self.sim.menu_car_pending = true;
        if self.audio.is_none() {
            self.menu_car_draw();
        }
    }

    /// The main menu's car render info (0x63A660 → 0x6381A0, its
    /// `Rand_Int(8)` at 0x6383A5): the start-up's last shared draw, after
    /// the sound's (**measured**, the sequence log: 0x4866F0, EA Trax's
    /// pick 0x46BA2F, then 0x6383A5). Once a process; the other front-end
    /// cars' render infos (the garage's, the shops', the car lots') are
    /// **open**.
    pub(crate) fn menu_car_draw(&mut self) {
        if std::mem::take(&mut self.sim.menu_car_pending) {
            nfsu2::rng_sites::render_info(&mut self.rng);
        }
    }

    /// The render infos of `n` world cars (0x63A5B0 → 0x6381A0 at
    /// 0x6383A5, one `Rand_Int(8)` each; **measured**, the sequence log).
    pub(crate) fn render_infos(&mut self, n: usize) {
        for _ in 0..n {
            nfsu2::rng_sites::render_info(&mut self.rng);
        }
    }

    /// A world build's draws (World_Start 0x60A690 → the world's
    /// constructor 0x5FF4A0; every free-roam entry, race load and restart),
    /// in the original's order: the world generator's seed (0x5FF557), the
    /// traffic manager's slot reset with the world (0x4179F0 → 0x412840:
    /// 15 × {paint, shared, world}), the rain set-up of both views
    /// (0x5FF6A5 → 0x61E100: 3260 shared draws). Returns the world
    /// generator. **Measured**: ≈ 3341 shared draws a build (c1/d1 `entry`,
    /// `setup`). The callers then make, in the sequence log's order
    /// (docs/original/rng.md "World build"): the late in-game audio load's draws
    /// ([`Game::audio_load_late`]), the render infos of the player and the
    /// other racers, the traffic fill (world generator; a render info per
    /// car placed), the career's slot-event picks (free roam, 0x527D40),
    /// CARSFX_Stereo's rolls, Traffic_AssignModels (0x412C9A),
    /// SfxCtlEngine's draws and the placed traffic cars' render infos again
    /// ([`Game::traffic_models_draws`]).
    pub(crate) fn world_build(&mut self) -> nfsu2::opponents::GameRng {
        let mut world = nfsu2::rng_sites::world_seed(&mut self.rng);
        nfsu2::rng_sites::model_slots(&mut self.rng, Some(&mut world), &self.sim.traffic_paints);
        if let Some(d) = self.sim.drive.as_mut() {
            d.weather.rain_init(&mut self.rng);
        }
        // The harness's traffic knob varies the world's stream.
        world.0 ^= traffic_seed();
        world
    }

    /// The in-game audio load (0x57DDA6 → 0x486EA0 → Audio_LoadInGame
    /// 0x486C10) when it comes before the world build: with the sound on,
    /// the speech library's re-seed (MixMap_Select's `Rand_Int(0xFFFFFFFF)`
    /// 0x486A2C, [`Game::speech_seed`]), then in a race the traffic sound's
    /// set-up (0x464217) and the accel transitions (0x45CADE × 3).
    /// **Measured** (the sequence log): first free-roam entry from the
    /// front end and a drag race from free roam: here, after the line-up
    /// and before the world generator's seed; a circuit race from free
    /// roam: no re-seed, and the other two after the rain set-up
    /// ([`Game::audio_load_late`]). Taken as the mix map's kind (`kind`,
    /// 0x4869A0: drag 2, drift 1, else 0) changing since the last load
    /// (**inferred**: what makes the load step skip MixMap_Select is not
    /// traced). Returns whether it ran.
    pub(crate) fn audio_load_early(&mut self, kind: u8, race: bool) -> bool {
        let Some(a) = self.audio.as_mut() else { return false };
        if a.mix_kind == Some(kind) {
            return false;
        }
        a.mix_kind = Some(kind);
        self.speech_seed();
        self.audio_objects_draws(race);
        true
    }

    /// The in-game audio objects' draws after the rain set-up, when
    /// [`Game::audio_load_early`] did not run (sound on only).
    pub(crate) fn audio_load_late(&mut self, early: bool, race: bool) {
        if !early && self.audio.is_some() {
            self.audio_objects_draws(race);
        }
    }

    fn audio_objects_draws(&mut self, race: bool) {
        if race {
            nfsu2::rng_sites::audio_setup::traffic_add(&mut self.rng);
        }
        nfsu2::rng_sites::audio_setup::accel_trans(&mut self.rng);
    }

    /// CARSFX_Stereo's nine rolls (0x4765F5) after the traffic fill (and
    /// in free roam the slot events), sound on only; the racers' stereo
    /// objects take them ([`nfsu2::audio::collision::set_stereo_rolls`]).
    pub(crate) fn stereo_rolls(&mut self) {
        if self.audio.is_some() {
            let rolls = nfsu2::rng_sites::audio_setup::stereo_rolls(&mut self.rng);
            nfsu2::audio::collision::set_stereo_rolls(&rolls);
        }
    }

    /// After Traffic_AssignModels (0x412C9A): SfxCtlEngine's draws (sound
    /// on) and the placed traffic cars' render infos again, their models
    /// given (**measured**: as many as the fill placed).
    pub(crate) fn traffic_models_draws(&mut self, placed: usize) {
        if self.audio.is_some() {
            nfsu2::rng_sites::audio_setup::engine_controls(&mut self.rng);
        }
        self.render_infos(placed);
    }

    /// The speech library's init (0x486220) on the game's draw
    /// `Rand_Int(0xFFFFFFFF)`: at the audio start (0x4866F0, made at the
    /// first audio frame: nothing draws between the start-up and the main
    /// menu) and at an in-game audio load (MixMap_Select 0x486A2C,
    /// [`Game::audio_load_early`]). Only with the sound on (no audio, no
    /// draw: **measured**, c1 sound off).
    pub(crate) fn speech_seed(&mut self) {
        if let Some(a) = self.audio.as_mut() {
            if !a.speech_seeded {
                a.speech_seeded = true;
                a.streams.init_speech_library(self.rng.below(0xFFFF_FFFF));
            }
            a.streams.init_speech_library(self.rng.below(0xFFFF_FFFF));
        }
    }

    /// The audio start's speech init (0x4866F0) once, at the first audio
    /// frame.
    pub(crate) fn speech_start(&mut self) {
        if let Some(a) = self.audio.as_mut()
            && !a.speech_seeded
        {
            a.speech_seeded = true;
            a.streams.init_speech_library(self.rng.below(0xFFFF_FFFF));
        }
    }

    /// Renderers for traffic cars the load's fill placed (the frame makes
    /// a car's renderer where it first draws it, and the tick steps a car's
    /// wet state only once it has one: made here, the cars on the road at
    /// the first tick step it from that tick at every display rate).
    pub(crate) fn placed_traffic_renderers<'a>(
        &mut self,
        cars: impl Iterator<Item = (&'a crate::sim::car_id::DrawId, &'a Car)>,
    ) {
        let Some(w) = self.platform.window.as_ref() else { return };
        let opts = nfsu2::render::RenderOptions::in_force();
        for (id, car) in cars {
            if self.render.cars.get(id).is_none() {
                self.render.cars.set(id, CarRenderer::new(&w.gpu, car, w.config.format, &opts));
            }
        }
    }
}
