use crate::*;
use crate::sim::session::{RaceTick, SceneTick, SimSound, TickEvents, WorldMode};

mod explore;
mod fe;
mod input;

/// What the frame's input phase hands the later phases (`tick_input`;
/// the simulation's own input is `sim::session::TickInput`).
#[derive(Clone, Copy)]
pub(crate) struct TickControls {
    /// This frame's driving controls (the script's hold and the
    /// hydraulics applied).
    pub(crate) controls: Controls,
}

/// Whether `update_world` goes on after a phase, or ends the update there
/// (the body's early `return`s, which skip the camera's velocity at its
/// end).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Flow {
    Go,
    Stop,
}

impl Game {
    /// A simulation update (`update_world`), and its state hash line when
    /// `NFSU2_STATE_HASH` is set (the parity harness's state gate; `state_hash`).
    pub(crate) fn update(&mut self, dt: f32) -> Result<()> {
        // Every dt a tick's (dev-harness and test builds).
        dev_hooks::check_tick_dt(self, dt);
        // The real-time clock and the quantised dt as this tick reads
        // them, from its first step (the input's key handlers open screens
        // whose C++ timers stamp 0x8651A8; `GameClock::real_tick`).
        self.pacing.clocks.real_tick = self.pacing.clocks.real;
        self.pacing.clocks.dt = dt;
        // The window's key and mouse events due at this
        // tick, in their order, before anything of the tick runs (the
        // original reads its input after the frame clock, before the world
        // update: 0x5811D0 → 0x5809D0).
        self.apply_input_queue();
        // The world ran this tick unless a menu stands over a loaded
        // world with no scene stepping (the pause: 0x601A70(0) sets the
        // world dt 0x865150 = 0, so 0x57A0F0 steps neither the sim clock
        // nor the world counter). The front end
        // without a world sets 0x865150 = 0x865194 (0x581469): it runs.
        let world_loaded = self.sim.drive.is_some();
        let world_ran = !(world_loaded && matches!(self.mode, Mode::Menu(_)) && self.scenes.shop.is_none());
        self.pacing.clocks.begin();
        // The audio's tick clock: the lines' "over" reads it.
        self.audio_tick_begin(dt);
        let r = self.update_world(dt);
        // The EA Trax caption's tick (0x4DB9C0): its GO and end on
        // the real-time clock.
        let (real, now) = (self.real_now(), self.now());
        if let Some(c) = self.platform.chyron.as_mut()
            && !c.step(real, now)
        {
            self.platform.chyron = None;
        }
        dev_hooks::record_state(self, dt);
        // The game's clocks at the tick's end (0x5663C0 the package clock,
        // 0x57A060 / 0x57A0F0 the 4000 Hz clocks): the package
        // step and the counters are the next tick's and the draw's.
        self.pacing.clocks.tick(dt, world_ran, world_loaded && self.sim.drive.is_some());
        // The Enhanced effects' simulations, once per tick from
        // the tick's state (the window's ticks and the harness's alike).
        let span = nfsu2::prof::acc::span("tick: enhanced fx");
        self.enh_fx_tick(dt);
        drop(span);
        // A headless audio capture (`NFSU2_AUDIO_WAV`; `dev_hooks`) driven
        // by `update` alone: the tick's sounds as `step` posts them after
        // the update, then its frames rendered (under `step` its
        // `audio_frame` does both).
        dev_hooks::audio_capture_frame(self, dt);
        r
    }

    /// The world's update: its phases in order. A phase
    /// returning `Flow::Stop` ends the update there.
    fn update_world(&mut self, dt: f32) -> Result<()> {
        let input = self.tick_input(dt)?;
        // The settings this tick reads, once, with ALL of the tick's input
        // applied (`sim::TickSettings`): the key queue above, and the pads
        // and the mouse in `tick_input`. An option changed by any of them
        // is in force in this same tick.
        self.pacing.tick_settings = crate::sim::TickSettings::take(&self.settings.custom);
        self.tick_fe_lines();
        self.tick_showcase(dt, &input);
        // The simulation's tick (`sim::Session::step`): the world's
        // animations, the camera director's draw (in the world update
        // before the frame's rain: 0x5FB060 in the world's update, 0x61F130
        // in the draw) and a race's update; its events handed on
        // where the update acted on them before, in their order.
        let span = nfsu2::prof::acc::span("tick:   sim step");
        let mut events = self.sim_step(dt, input.controls);
        drop(span);
        // The tick's contact sounds into the session's queues (the
        // audio's `impact_sounds` takes them), in their order.
        self.sim.contact_sounds.append(std::mem::take(&mut events.contacts));
        self.sim_sounds(&events.sounds);
        let flow = match events.race {
            Some(tick) => self.race_flow_event(tick),
            None => match &mut self.mode {
                // The shop scene's frame ran in the session; its end
                // is the flow's.
                Mode::Explore if events.scene.is_some() => {
                    if let Some(SceneTick::Ended(run)) = events.scene {
                        self.shop_scene_end(*run, dt);
                    }
                    Flow::Go
                }
                Mode::Explore => {
                    let world_dt = events.roam_dt.unwrap_or(dt);
                    let _span = nfsu2::prof::acc::span("tick:   free roam");
                    self.update_explore(dt, world_dt, &input, events.encounters.unwrap_or_default())
                }
                Mode::Race(_) => unreachable!("a race ticks in the session"),
                Mode::Menu(_) => {
                    self.update_menus(dt)?;
                    Flow::Go
                }
            },
        };
        if flow == Flow::Stop {
            return Ok(());
        }
        self.tick_camera_motion(dt);
        Ok(())
    }

    /// The simulation's tick: its input built (the settings it
    /// reads copied, the AI data and route triggers shared) and the
    /// session lent (disjoint fields: the drive, the race, the scene and
    /// its crowd, the shared generator), then [`sim::session::Session::step`].
    fn sim_step(&mut self, dt: f32, controls: Controls) -> TickEvents {
        let world = match &self.mode {
            Mode::Menu(_) => WorldMode::Menus,
            Mode::Explore => WorldMode::Explore,
            Mode::Race(_) => WorldMode::Race,
        };
        // 0x513D30 / 0x513E00: no outrun offered or engaged while a world
        // challenge runs (career +0xC08 = 0x85B948 set).
        let challenge = self.career_world.challenge.is_some() && self.profile.is_some() && !self.race_flow.quick_free_run;
        let roam = sim::session::RoamInput {
            profile: self.profile.as_ref(),
            career: &self.game_data.career,
            tricks: &self.game_data.tricks,
            metric: self.settings.metric,
            challenge,
        };
        let input = sim::session::TickInput {
            dt,
            world,
            controls,
            lap_info: self.settings.players[0].lap_info,
            camera: self.settings.players[0].camera,
            settings: self.pacing.tick_settings,
            ai: self.sim.ai.as_ref(),
            route_triggers: &self.sim.route_triggers,
            roam: Some(&roam),
        };
        let mut session = sim::session::Session {
            drive: &mut self.sim.drive,
            race: match &mut self.mode {
                Mode::Race(race) => Some(race),
                _ => None,
            },
            scene: &mut self.scenes.shop,
            crowd: &mut self.scenes.crowd,
            encounters: &mut self.sim.encounters,
            traffic: &mut self.sim.traffic,
            roam_style: &mut self.sim.roam_style,
            roam_camera: &mut self.sim.roam_camera,
            rng: &mut self.rng,
        };
        session.step(&input)
    }

    /// Free roam's second phase (`Session::step_roam`), after the
    /// outrun's career consequences: the car, the traffic and the
    /// free-roam style manager; a won outrun's bank on the race-over
    /// message first.
    fn sim_roam(&mut self, dt: f32, controls: Controls, result_bank: Option<i32>) {
        let roam = sim::session::RoamInput {
            profile: self.profile.as_ref(),
            career: &self.game_data.career,
            tricks: &self.game_data.tricks,
            metric: self.settings.metric,
            challenge: self.career_world.challenge.is_some() && self.profile.is_some() && !self.race_flow.quick_free_run,
        };
        let input = sim::session::TickInput {
            dt,
            world: WorldMode::Explore,
            controls,
            lap_info: self.settings.players[0].lap_info,
            camera: self.settings.players[0].camera,
            settings: self.pacing.tick_settings,
            ai: self.sim.ai.as_ref(),
            route_triggers: &self.sim.route_triggers,
            roam: Some(&roam),
        };
        let mut session = sim::session::Session {
            drive: &mut self.sim.drive,
            race: None,
            scene: &mut self.scenes.shop,
            crowd: &mut self.scenes.crowd,
            encounters: &mut self.sim.encounters,
            traffic: &mut self.sim.traffic,
            roam_style: &mut self.sim.roam_style,
            roam_camera: &mut self.sim.roam_camera,
            rng: &mut self.rng,
        };
        let events = session.step_roam(&input, result_bank);
        self.sim.contact_sounds.append(events.contacts);
        self.sim_sounds(&events.sounds);
    }

    /// The session lent outside the tick's `sim_step` (a shop
    /// scene's frame at its start or skip, `shop_scene_frame`): the same
    /// disjoint fields.
    pub(crate) fn sim_session(&mut self) -> sim::session::Session<'_> {
        sim::session::Session {
            drive: &mut self.sim.drive,
            race: match &mut self.mode {
                Mode::Race(race) => Some(race),
                _ => None,
            },
            scene: &mut self.scenes.shop,
            crowd: &mut self.scenes.crowd,
            encounters: &mut self.sim.encounters,
            traffic: &mut self.sim.traffic,
            roam_style: &mut self.sim.roam_style,
            roam_camera: &mut self.sim.roam_camera,
            rng: &mut self.rng,
        }
    }

    /// A race tick's end handed to the flow where the race's update
    /// called it before: the race over (`finish_race`), a replay left
    /// (`end_replay`).
    fn race_flow_event(&mut self, tick: RaceTick) -> Flow {
        match tick {
            RaceTick::Go => Flow::Go,
            RaceTick::Stop => Flow::Stop,
            RaceTick::Finished { time } => self.finish_race(RaceEnd { time }),
            RaceTick::ReplayLeft => {
                self.end_replay();
                Flow::Stop
            }
        }
    }

    /// The front end's update: the dyno, the menus' ticks and the movies
    /// (the ending scene behind the post-race screens is the session's).
    fn update_menus(&mut self, dt: f32) -> Result<()> {
        // The race's ending scene behind the post-race screens ran
        // in the session (`Session::ending_scene`); it never ends there.
        self.dyno_tick(dt);
        self.menu_tick();
        self.movie_tick(dt)?;
        Ok(())
    }

    /// The sounds the simulation asked for in the tick, handed to
    /// the audio where the update set them before (the tick's sound list
    /// posts them, `audio_sounds`).
    fn sim_sounds(&mut self, sounds: &[SimSound]) {
        for sound in sounds {
            match sound {
                SimSound::CrashCamera => {
                    if let Some(a) = self.audio.as_mut() {
                        a.soundfx.crash_camera = true;
                    }
                }
            }
        }
    }

    /// The camera's velocity, at the update's end.
    fn tick_camera_motion(&mut self, dt: f32) {
        // The camera's velocity (camera +0x1E0, 0x4474A4).
        if let Some(d) = self.sim.drive.as_mut() {
            d.motion.camera(d.camera.position, dt);
        }
    }
}
