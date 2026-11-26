use crate::*;

impl Game {
    /// The tuning session's entry (0x5309F0 → 0x526980(1)): the dyno
    /// track (0x4FF160: 4706 "Speedway Dyno Testing", race type 8, no
    /// laps) is loaded with the car on it, and once in (0x60B52C, the
    /// session flag 0x89E7E6 and 0x83A7A4 set) the game pushes
    /// UI_PerformanceTuning_Setting while no mode is chosen (0x510060:
    // prov: inferred game.enter_dyno
    /// 0x83A63C = 0xFF; with 0x83A655 clear, 0x510080, **inferred** clear
    /// here), else UI_PerformanceDyno_MAIN (0x7906F0). Seen under Wine
    /// (menus112): Customize's Yes opens Mode Select over the car on the
    /// dyno.
    pub(crate) fn enter_dyno(&mut self) -> Mode {
        self.dyno.tuning_session = true;
        // Entering the dyno (0x50F800): the working settings are the car
        // object's (`tuning_entry`, for Quit Tuning's 0x50FD10).
        if let Some(p) = self.profile.as_ref() {
            self.dyno.tuning_entry = p.tuning;
        }
        // The dyno menu's tuning object (0x50FE20 → 0x516AC0) is the car
        // built with settings 0, so its sliders are those settings as the
        // set-up's clamps (0x5B3EF0) stored them back.
        self.dyno.tuning_object = self.profile.as_ref().map(|p| self.clamped_tuning(p.tuning[0]));
        if let Err(e) = self.load_dyno_track() {
            eprintln!("dyno track: {e:#}");
        }
        self.dyno_menu()
    }

    /// The tuning session's first menu (0x60B52C): Mode Select with no
    /// mode chosen, else the dyno menu.
    pub(crate) fn dyno_menu(&mut self) -> Mode {
        self.icon_menu(if self.dyno.tuning_mode == 0xFF { MenuKind::TuneMode } else { MenuKind::Dyno })
    }

    /// The dyno entry's Yes in play: the track, then the DynoEnter scene
    /// (Wine menus113 e1–e8: the car comes down the pit lane and up onto
    /// the rollers, about 8 s), whose end puts up the first menu. The
    /// scene's type 2 takes its origin as a race start scene does
    /// (0x43B000 → 0x431990 with the first flag, as type 0); the clone
    // prov: inferred game.enter_dyno_scene
    /// stands it on the dyno's grid (**inferred**: the track has no route,
    /// so the marker 0x431990 reads there is not traced). Enter skips it
    /// (agreed deviation, docs/playing/native-deviations.md "Skipping scenes").
    pub(crate) fn enter_dyno_scene(&mut self) -> Mode {
        let menu = self.enter_dyno();
        self.dyno_enter_scene(menu)
    }

    /// DynoEnter on the loaded dyno track (`menu` without it). Also after
    /// a test run's Return to Dyno (**observed** under Wine: the
    /// session's reload put the car on mode 5 under 0xB3B2B197 for ~9 s
    /// before the Dyno Shop came back; the maintainer's private captures).
    pub(crate) fn dyno_enter_scene(&mut self, menu: Mode) -> Mode {
        use nfsu2::shop_scene::{Scene, SceneKind, origin};
        let Some(d) = self.sim.drive.as_ref() else { return menu };
        let scene = match Scene::load(&self.game_data.root, SceneKind::DynoEnter) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("dyno scene: {e:#}");
                return menu;
            }
        };
        // The origin (**verified**): 0x43B000 calls 0x431990 for type 2
        // with param_4 1 and no first flag, so 0x42FC60 finds the track's
        // "ForwardStartLine" (0x89E7A4 clear; 0x86501C unset) and returns
        // its +0x20 + 0x4000, which 0x431990 turns back by 0x4000: the
        // start line's place (ground z) and heading, as a race intro's.
        let Some((line, heading)) = self.dyno_start_line() else { return menu };
        let ground = Vec3::new(line.x, line.y, d.world.grid_height(line));
        let camera = scene.camera_track(&nfsu2::shop_scene::load_cameras(&self.game_data.root)).cloned();
        self.scenes.shop = Some(ShopRun {
            audio: nfsu2::shop_scene::audio::Run::with_carry(self.pacing.cadence.clock.carry),
            companion: None,
            car_slot: 1,
            scene,
            shop: None,
            origin: origin(ground, heading),
            frame: 0.0,
            events: 0,
            key: 0,
            camera,
        });
        // A new scene's slot starts at count 0 (0x43B172): its first
        // target places the car (0x4309D0 → 0x5FBD70).
        if let Some(d) = self.sim.drive.as_mut() {
            d.state.nis = None;
        }
        if let Some(d) = self.sim.drive.as_mut() {
            d.state.held = false;
        }
        self.shop_scene_frame(0.0);
        Mode::Explore
    }

    /// The Dyno Shop's Test Run on the dyno (**observed** under Wine,
    /// the finished-career save's Lancer, Circuit: as Test Run was picked, 0x827E1C became
    /// BinHash("DynoExit") 0xB7DEC9B3 and the car went on mode 5 for 6 s,
    /// off the rollers and down the ramp, before the test run's track
    /// 4705 loaded with race type 1; the maintainer's private captures).
    /// **Verified**: the Test Run ends the dyno session's race (race type
    /// 8, 0x89E7E6 set): the race flow's mode 7 handler 0x603EC0 (state
    /// table 0x80214C entry 7) case 1, with +0x44 set (0x60A878 at the
    /// race's start) and 0x4018F0 false (race +0x38 clear, 0x890108 = 0),
    /// takes race type 8 past the dyno-session return (only types ≠ 8 go
    /// back to the tuning menu, 0x603640(0) / 0x601030(10)) to +8 = 5 and
    /// 0x5F41F0(4, 0), whose scene pick 0x436840 case 1 gives DynoExit on
    /// the dyno track 0x1262. Its origin is DynoEnter's (type 2:
    /// 0x43B000 → 0x431990 → 0x42FC60, the dyno's ForwardStartLine).
    /// Returns false (no scene) off the dyno track.
    pub(crate) fn exit_dyno_scene(&mut self) -> bool {
        use nfsu2::shop_scene::{Scene, SceneKind, origin};
        if !self.on_dyno_track() || TEST_RUNS.get(self.dyno.tuning_mode as usize).is_none() {
            return false;
        }
        let Some(d) = self.sim.drive.as_ref() else { return false };
        let scene = match Scene::load(&self.game_data.root, SceneKind::DynoExit) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("dyno exit scene: {e:#}");
                return false;
            }
        };
        let Some((line, heading)) = self.dyno_start_line() else { return false };
        let ground = Vec3::new(line.x, line.y, d.world.grid_height(line));
        let camera = scene.camera_track(&nfsu2::shop_scene::load_cameras(&self.game_data.root)).cloned();
        self.scenes.shop = Some(ShopRun {
            audio: nfsu2::shop_scene::audio::Run::with_carry(self.pacing.cadence.clock.carry),
            companion: None,
            car_slot: 1,
            scene,
            shop: None,
            origin: origin(ground, heading),
            frame: 0.0,
            events: 0,
            key: 0,
            camera,
        });
        // A new scene's slot starts at count 0 (0x43B172); the car leaves
        // the dyno session's hold.
        if let Some(d) = self.sim.drive.as_mut() {
            d.state.nis = None;
            d.state.held = false;
        }
        self.scene_line("DynoExit", 0);
        self.shop_scene_frame(0.0);
        true
    }

    /// The dyno track's ForwardStartLine marker (place and heading as the
    /// markers give them; 0x5D8670 by name).
    pub(crate) fn dyno_start_line(&self) -> Option<(Vec3, f32)> {
        let track = self.game_data.career.tracks.get(&DYNO_TRACK)?;
        let markers =
            career::load_markers(&nfsu2::game_fs::path(&self.game_data.root, &format!("TRACKS/ROUTES{}/TrackPosMarkersAll.bin", track.region)))
                .ok()?;
        let line = nfsu2::formats::hash::bin_hash("ForwardStartLine");
        let m = markers.iter().find(|m| m.track == DYNO_TRACK && m.name == line)?;
        Some((m.position, m.heading))
    }

    /// The dyno track 4706 with the car on its grid and the dyno camera.
    pub(crate) fn load_dyno_track(&mut self) -> Result<()> {
        // The track has no paths file (no route): its grid is the
        // markers' ForwardStartingPosition slot 0 (0x5EB5E0), on the
        // rollers (TrackPosMarkersAll.bin of L4RG: (-1682.30, -1957.06,
        // 43.23), heading 21844).
        let track = self.game_data.career.tracks.get(&DYNO_TRACK).context("dyno track 4706")?.clone();
        let markers =
            career::load_markers(&nfsu2::game_fs::path(&self.game_data.root, &format!("TRACKS/ROUTES{}/TrackPosMarkersAll.bin", track.region)))?;
        let start = nfsu2::formats::hash::bin_hash("ForwardStartingPosition");
        let m = markers
            .iter()
            .filter(|m| m.track == DYNO_TRACK && m.name == start)
            .min_by_key(|m| m.slot)
            .context("the dyno's grid")?;
        let (at, heading) = (m.position, m.heading);
        self.sim.traffic = None;
        // Free roam's encounter racers stay in the city: the dyno track is
        // another region, and its scenes run in Explore mode, which draws
        // (and steps) them; their ground state is the city's (#236: a
        // pack index past the dyno's). The city start makes them anew.
        self.sim.encounters = None;
        let stage = self.profile.as_ref().map_or(0, |p| p.stage);
        let owns_car = self.profile.as_ref().is_some_and(|p| !p.car.is_empty());
        let barriers = BarrierSetup {
            track: DYNO_TRACK as i32,
            free_roam: false,
            career: false,
            stage,
            reversed: false,
            owns_car,
        };
        self.drive_in(&track.region, at, heading, RaceMode::FreeRoam, DYNO_SLIDERS, barriers)?;
        if let Some(d) = self.sim.drive.as_mut() {
            let z = d.world.grid_height(at);
            d.place(Vec3::new(at.x, at.y, z), heading);
            d.state.held = true;
            d.camera = dyno_camera(d.state.position, d.state.orientation, DYNO_DISTANCE.1);
        }
        self.dyno.distance = DYNO_DISTANCE.1;
        // The manager is made with the dyno race's car (0x5FC4E5, race
        // type 8), which places its rollers under the car on its grid
        // (0x5FC4FF → 0x56D1B0): they stand there through the DynoEnter
        // scene that brings the car in.
        self.seat_on_dyno(true);
        Ok(())
    }

    /// The car on the dyno's grid: its place is the rest pose the pull is
    /// drawn from, and with `rollers` the rollers are placed under it
    /// (0x56D1B0).
    pub(crate) fn seat_on_dyno(&mut self, rollers: bool) {
        let Some(d) = self.sim.drive.as_ref() else { return };
        self.dyno.rest = Some((d.state.position, d.state.orientation));
        if rollers {
            let com = d.state.position + d.state.orientation * Vec3::from(d.car.car_type.handling.centre_of_mass);
            let world = &d.world;
            self.dyno.rollers =
                Some(nfsu2::dyno::Rollers::place(&d.car.car_type, com, d.state.orientation, |p| world.grid_ground(p)));
        }
    }

    /// The dyno's car drawn as the pull moves it (the game's dyno race
    /// drives the car on the rollers itself: the manager is its
    /// controller, 0x6016B0 → 0x56E6F0): its wheels turn and spin, its
    /// springs work, its body tilts about its centre of mass, its engine
    /// revs, and the rollers turn with the wheels. Under Wine (the
    /// finished-career save's Lancer) the body stayed within 4 cm of its place, rose 3 mm
    /// and tilted about 0.6° nose up under the pull and 0.9° down under
    /// the brake (car +0x90 forward z 0.0106 / −0.0153), the four wheels
    /// spinning to 162 rad/s. The clone runs the pull on a free car
    /// ([`nfsu2::dyno::DynoPull`]) and carries its movement over to the
    /// car on the rollers (**inferred** equivalent of the game's held car:
    /// the position spring 0x5A5C30 that keeps the body in place is not
    /// ported).
    pub(crate) fn dyno_pose(&mut self, dt: f32) {
        let pull = match &self.mode {
            Mode::Menu(Menu::DynoChart(c)) => c.pull.as_ref(),
            _ => self.dyno.spin.as_ref(),
        };
        let Some((pose, drivetrain)) =
            pull.filter(|p| p.stage() != 0).map(|p| (p.pose(), p.car_state().drivetrain.clone()))
        else {
            return;
        };
        if !self.on_dyno_track() {
            return;
        }
        let Some((origin, turn)) = self.dyno.rest else { return };
        let Some(d) = self.sim.drive.as_mut() else { return };
        let com = Vec3::from(d.car.car_type.handling.centre_of_mass);
        let at = origin + turn * com + Vec3::Z * pose.rise;
        let turned = (turn * pose.tilt).normalize();
        d.state.orientation = turned;
        d.state.position = at - turned * com;
        for (i, w) in d.state.wheels.iter_mut().enumerate() {
            w.roll = pose.roll[i];
            w.spin = pose.spin[i];
            w.compression = pose.compression[i];
        }
        if drivetrain.is_some() {
            d.state.drivetrain = drivetrain;
        }
        if let Some(r) = self.dyno.rollers.as_mut() {
            r.turn_by(pose.spin, dt);
        }
    }

    /// Whether the drive is the dyno track's (its region).
    pub(crate) fn on_dyno_track(&self) -> bool {
        let region = self.game_data.career.tracks.get(&DYNO_TRACK).map(|t| t.region.as_str());
        self.sim.drive.as_ref().is_some_and(|d| Some(d.region.as_str()) == region) && self.sim.paused_race.is_none()
    }

    /// Dyno Run (0x53C840): UI_PerformanceDyno_Chart for the career car.
    /// The dyno manager drives the career car with the dyno's settings
    /// (0x526980(1): the mode's settings, the sliders copied over). The
    /// chart's lines (0x53D0B0) read the player racer's engine (0x8900AC
    /// [0x83A64C] → +0x4 → +0x20: idle +0x1F0, max +0x1F8, torque +0x200),
    /// the tuned car's, so they follow the ECU / turbo tuning.
    pub(crate) fn dyno_chart(&self, now: f32) -> Mode {
        let Some(h) = self.dyno_handling() else { return self.icon_menu(MenuKind::Dyno) };
        let first = self.dyno.first.clone();
        let (name, _, perf) = self.player_car();
        let perf = self.with_uniques(&perf);
        let settings = self.dyno_settings();
        let pull = Car::load_physics(
            &self.game_data.root,
            &name,
            &self.game_data.career.packages,
            &|id| perf.contains(&id),
            Some((&self.game_data.career.tuning, &settings)),
        )
        .ok()
        .map(|car| nfsu2::dyno::DynoPull::new(car).with_stability_off(self.settings.players[0].stability_off));
        Mode::Menu(Menu::DynoChart(screens::DynoChart {
            engine: h.engine,
            first,
            metric: self.settings.metric,
            opened: now,
            recorded: false,
            pull,
            finished: None,
        }))
    }

    /// The chart's frame: the pull runs on, and at its end the run is
    /// recorded (0x54A810 → 0x523340, once).
    pub(crate) fn dyno_tick(&mut self, dt: f32) {
        let now = self.now();
        match &mut self.mode {
            Mode::Menu(Menu::DynoChart(chart)) => {
                self.dyno.spin = None;
                chart.step(dt, now);
                // The pull's contact sounds into the session's queues
                // (its settling steps' at `dyno_chart` too: posted in this
                // tick's input phase, nothing posted since).
                if let Some(p) = chart.pull.as_mut() {
                    self.sim.contact_sounds.append(std::mem::take(&mut p.sounds));
                }
                if chart.progress() >= 1.0 {
                    self.record_dyno_run();
                }
            }
            Mode::Menu(Menu::DynoResults(_)) => {
                if let Some(p) = self.dyno.spin.as_mut() {
                    p.step(dt);
                    // Its contact sounds into the session's queues.
                    self.sim.contact_sounds.append(std::mem::take(&mut p.sounds));
                }
            }
            _ => {
                self.dyno.spin = None;
                return;
            }
        }
        self.dyno_pose(dt);
    }

    /// 0x523340 for the chart on screen, once.
    pub(crate) fn record_dyno_run(&mut self) {
        let Mode::Menu(Menu::DynoChart(chart)) = &mut self.mode else { return };
        if chart.recorded {
            return;
        }
        chart.recorded = true;
        let engine = chart.engine.clone();
        if self.dyno.first.is_none() {
            self.dyno.first = Some(engine);
        }
        // 0x523340 reads the tuning object's car set-up ([[state] +0x10]
        // +0x10, 0x5B5360(1) → its block +0x30): the peaks 0x5B56F0 /
        // 0x5B5820, the torque values and the top speed 0x5B5940 come from
        // the block the sliders tune.
        if let Some(mut h) = self.dyno_handling() {
            nfsu2::handling::setup_performance(&mut h, RaceMode::FreeRoam);
            self.dyno.runs.record(screens::DynoRun::of(&h));
        }
    }
}
