use crate::*;

impl Game {
    /// 0x4995A0, the drive-in's end (or the entry with no scene): with
    /// the first-entry flag 0x836350 set, 0x557E00 clears the movie queue,
    /// puts the shop's movie (+0x20: SCENE05 for CC_CAR_LOT_1, SCENE06 for
    /// CRIB_1, no other shop has one) in it, game mode 0x838568 = 2, and
    /// pushes IG_PlayMovie.fng; the queue's end runs 0x54AF90 case 2 →
    /// 0x499560: the shop's front end (0x528570) and 0x836350 = 0. Without
    /// the flag the front end at once (0x528570).
    pub(crate) fn open_shop(&mut self, index: usize) -> Mode {
        self.reset_car_sound_context();
        if std::mem::take(&mut self.shop.movie) {
            let name = self.game_data.career.shops[index].movie.to_lowercase();
            let mut m = MovieScreen::new(&self.game_data.root, 2, &[name.as_str()]);
            m.shop = Some(index);
            if let Some(a) = self.audio.as_ref() {
                a.mixer.set_stream(None);
            }
            return Mode::Menu(Menu::Movie(m));
        }
        self.shop_front_end(index)
    }

    /// The visited shop's kind (0x86344C +0x50), if any.
    pub(crate) fn visited_shop_kind(&self) -> Option<u8> {
        self.shop.visit.and_then(|h| self.game_data.career.shops.iter().find(|s| s.hash == h)).map(|s| s.kind)
    }

    /// The shop's front end (0x4995A0 → 0x557E00 for the shop; the car
    /// lot 0x528570).
    pub(crate) fn shop_front_end(&mut self, index: usize) -> Mode {
        let shop = self.game_data.career.shops[index].clone();
        // 0x528570: in a world challenge's state 4 (its goal shop entered,
        // 0x54CB00) the front end is THE_UNIQUES_SHOP's (kind 6).
        if self.career_world.challenge.is_some_and(|c| c.state == 4) {
            return self.unique_part_screen();
        }
        // Their INIT posts Rachel's hints (see `update`, `fe_line`).
        // A shop is never the garage's storage.
        self.shop.garage_parts = false;
        // Entered from free roam: no Visual Rating widget up yet.
        self.fe.rating_up.set(false);
        match shop.kind {
            // "Enter Garage": the career garage.
            0 => self.icon_menu(MenuKind::Crib),
            // UI_CareerCarLot (0x528570).
            4 => self.career_lot(),
            2 => {
                self.shop.garage_parts = false;
                self.new_parts_browser(0)
            }
            1 => self.icon_menu(MenuKind::Graphics),
            3 => self.perf_category_menu(0),
            5 => self.specialties_browser(0, None),
            // Kind 6, THE_UNIQUES_SHOP (0x50E2E0 → UI_ChooseUniquePart).
            _ => self.unique_part_screen(),
        }
    }

    /// Start a shop's drive-in or drive-out scene (`nfsu2::shop_scene`):
    /// at the free-roam marker named by the shop's hash (0x431930 /
    /// 0x42FC60 with 0x86501C), on the ground there (0x5E7E80), turned by
    /// its heading; the door messages go to the world animations keyed by
    /// the shop's world trigger (0x827ED4). False when there is no scene,
    /// marker or car.
    pub(crate) fn start_shop_scene(&mut self, kind: nfsu2::shop_scene::SceneKind, index: usize) -> bool {
        use nfsu2::shop_scene::{Scene, origin};
        let shop = &self.game_data.career.shops[index];
        let Some(d) = self.sim.drive.as_mut().filter(|d| d.region == EXPLORE_REGION) else { return false };
        let markers =
            career::load_markers(&nfsu2::game_fs::path(&self.game_data.root, &format!("TRACKS/ROUTES{EXPLORE_REGION}/TrackPosMarkersFreeRoam.bin")))
                .unwrap_or_default();
        let Some(marker) = markers.iter().find(|m| m.name == shop.hash) else { return false };
        let scene = match Scene::load(&self.game_data.root, kind) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("shop scene {}: {e:#}", kind.name());
                return false;
            }
        };
        let at = marker.position;
        let ground = Vec3::new(at.x, at.y, d.world.grid_height(at));
        let camera = scene.camera_track(&nfsu2::shop_scene::load_cameras(&self.game_data.root)).cloned();
        let name = scene.kind.name();
        // The drive-in's pick (0x436840 at 0x4370E3): Rand_Int(count) on the
        // shared generator even with one scene to pick from (**measured**:
        // one draw on "Enter Shop", s2). The drive-out's pick is
        // not measured (open).
        if matches!(kind, nfsu2::shop_scene::SceneKind::Enter) {
            self.rng.below(1);
        }
        let run = ShopRun {
            audio: nfsu2::shop_scene::audio::Run::with_carry(self.pacing.cadence.clock.carry),
            companion: None,
            car_slot: 1,
            scene,
            shop: Some(index),
            origin: origin(ground, marker.heading),
            frame: 0.0,
            events: 0,
            key: shop.trigger,
            camera,
        };
        self.scenes.shop = Some(run);
        // A new scene's slot starts at count 0 (0x43B172): its first
        // target places the car (0x4309D0 → 0x5FBD70).
        if let Some(d) = self.sim.drive.as_mut() {
            d.state.nis = None;
        }
        // ShopEnter1 / ShopExit1: NIS_Select_End = (0x200, 0x4000 / 0x8000, 1).
        self.scene_line(&name, 0);
        self.shop_scene_frame(0.0);
        true
    }

    /// A frame of the shop scene: the scene clock at 30 frames a second,
    /// its door messages as their time comes, the car on its NIS mover
    /// towards its channel (the scene origin times the key; see below),
    /// and the scene's camera
    /// track in the scene's frame (`Scene::camera`; the chase camera when
    /// it has none). At the end a
    /// drive-in opens the shop (0x4995A0) and a drive-out gives the car
    /// back.
    pub(crate) fn shop_scene_frame(&mut self, dt: f32) {
        // The scene's step is the simulation's (`Session::scene_frame`);
        // its end comes back for the flow (`shop_scene_end`).
        let camera = self.settings.players[0].camera;
        if let Some(sim::session::SceneTick::Ended(run)) = self.sim_session().scene_frame(dt, camera) {
            self.shop_scene_end(*run, dt);
        }
    }

    /// A shop scene's end (`sim::session::SceneTick::Ended`, acted
    /// on where `shop_scene_frame` acted on it before): the flow it opens,
    /// then the run handed to the audio's tail on a stepping frame.
    pub(crate) fn shop_scene_end(&mut self, run: ShopRun, dt: f32) {
        // Type 3 (an entry) opens the shop (0x4995A0); type 4 (an exit,
        // and the career's start scene) gives the player input back
        // (0x8900AC +4 → +0x30, vtable +0x1C(0)) and aims the GPS at
        // DDAY_EVENT_B's goal while that is the intro's event (0x532E20 →
        // 0x532D90).
        match (run.scene.kind, run.shop) {
            (nfsu2::shop_scene::SceneKind::Enter, Some(shop)) => self.mode = self.open_shop(shop),
            // The car stays on the rollers under the dyno camera and the
            // tuning session's first menu comes up (0x60B52C). It stays
            // where its mover left it (0x437A00 places nothing; under Wine
            // the body did not move at the switch to mode 1), held
            // there for the session.
            (nfsu2::shop_scene::SceneKind::DynoEnter, _) => {
                if let Some(d) = self.sim.drive.as_mut() {
                    d.state.velocity = Vec3::ZERO;
                    d.state.spin = Vec3::ZERO;
                    d.state.angular_momentum = Vec3::ZERO;
                    d.state.held = true;
                    d.camera = dyno_camera(d.state.position, d.state.orientation, DYNO_DISTANCE.1);
                }
                self.seat_on_dyno(false);
                self.mode = self.dyno_menu();
            }
            // DynoExit's end: the test run's track loads (0x5F41F0's
            // ending flow, then the Test Run's race, race type 1).
            (nfsu2::shop_scene::SceneKind::DynoExit, _) => {
                if let Err(e) = self.start_test_run() {
                    eprintln!("test run: {e:#}");
                    self.mode = self.icon_menu(MenuKind::Dyno);
                }
            }
            _ if run.scene.scene_type == 4 && self.profile.is_some() && !self.race_flow.quick_free_run => {
                self.gps_to_intro(career::DDAY_EVENT_B)
            }
            _ => {}
        }
        if dt > 0.0 {
            self.scenes.audio_tail = Some(run);
        }
    }

    /// The career's start scene, "Career Start NIS" (`Scene_CareerNis1`):
    /// the city start 0x5404B0, before the intro movie has played this
    /// session (0x86347C) and with the D-Day script idle (career +0x713C =
    /// 0), flags the NIS manager (0x4305D0: 0x827E88 +0x60); the track
    /// loader's state 6 (0x57F9C4) then picks a scene (0x437270 →
    /// 0x436840 case 4, which takes CareerNis1, 0x865598, while the script
    /// is idle, 0x500AA0) and plays it. A type-4 scene: its origin is the
    /// start marker (0x42FC60: 0x86501C, never written, so the named
    /// marker `ForwardStartLine`, `BackwardStartLine` for a backward
    /// track; its heading + 0x4000, which 0x431990 takes off again), on
    /// the ground there. The car stands where the city start put it (the
    /// scene's one key is ForwardStartingPosition's place and heading in
    /// that frame) while the camera track flies over the airport (group
    /// 0x346D604C, "Track 0", world-space points). The script begins only
    /// after it (`career_roam` needs no scene).
    pub(crate) fn start_career_scene(&mut self) -> bool {
        use nfsu2::shop_scene::{Scene, SceneKind, origin};
        let Some(d) = self.sim.drive.as_ref().filter(|d| d.region == EXPLORE_REGION) else { return false };
        let markers =
            career::load_markers(&nfsu2::game_fs::path(&self.game_data.root, &format!("TRACKS/ROUTES{EXPLORE_REGION}/TrackPosMarkersFreeRoam.bin")))
                .unwrap_or_default();
        let Some(marker) = markers.iter().find(|m| m.name == nfsu2::formats::hash::bin_hash("ForwardStartLine")) else {
            return false;
        };
        let scene = match Scene::load(&self.game_data.root, SceneKind::Career) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("career scene: {e:#}");
                return false;
            }
        };
        let at = marker.position;
        let ground = Vec3::new(at.x, at.y, d.world.grid_height(at));
        let camera = scene.camera_track(&nfsu2::shop_scene::load_cameras(&self.game_data.root)).cloned();
        self.scenes.shop = Some(ShopRun {
            audio: nfsu2::shop_scene::audio::Run::with_carry(self.pacing.cadence.clock.carry),
            companion: None,
            car_slot: 1,
            scene,
            shop: None,
            origin: origin(ground, marker.heading),
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
        self.shop_scene_frame(0.0);
        // Rachel's line over the scene: CareerNis1's NIS stream (0x484AF4:
        // NIS_Select_End = (0x400, 0x4000, 1), kind 9).
        if let Some(a) = self.audio.as_mut() {
            a.streams.stop_song(&a.mixer);
            a.streams.play_career_nis(&a.mixer);
            // 0x484B45: then 0x5258C0 on Rachel's hints, the Career line
            // (0x8000, 0x20) on the speech stream (its own object, +4;
            // the NIS line is on +8, so both play).
            a.streams.say(&a.mixer, &nfsu2::audio::speech::lines::career(0x8000, 0x20));
        }
        true
    }

    /// The race's start scene (track loader 0x4371C9 → 0x436840, case 0
    /// for a race no other human is in — 0x5F0100 on 0x89CF48 — case 3
    /// otherwise; **verified** unless marked): with Crowds on (0x5BF6E0:
    /// g_CrowdsEnable 0x870CD0) a scene of the NIS table's first list
    /// (`IntroTable::race_candidates`: the track's mask, the human's grid
    /// slot, the racer count, the track's start kind) picked at random
    /// (0x43C1C0; the clone's own draw), played from the start line marker
    /// (0x431990) with its camera track "Track %d" of the entry's variant
    /// (+0x04), and its characters (`Game::scene_crowd`). The scene start
    /// 0x437910 sets race status +0xC = (mark 1 − mark 2) / 30 / 3 and
    /// queues the timed event "StartCountdown" (0x827AFC, 0x435EA0) at
    /// mark 2: the countdown runs from mark 2, three steps to the green
    /// light at mark 1, with no 3-2-1 shown (**observed**). Crowds off:
    /// no scene and the 0.9 s step. Cars follow `ZPM_Car<n>` channels
    /// through `scene_cars`, with native burnout/brake wheel controls.
    // prov: open game.race_start_scene
    /// **Open**: the complete NISMover pose/force solver and remaining
    /// event track (engine torque, nitrous, audio, hydraulics).
    pub(crate) fn race_start_scene(&mut self, track: &career::Track, race: &mut Race) -> Option<ShopRun> {
        use nfsu2::shop_scene::{IntroTable, Scene, SceneKind, origin};
        self.scenes.crowd = None;
        // A new scene's slots start at count 0 (0x4391B0 / 0x43B172).
        if let Some(d) = self.sim.drive.as_mut() {
            d.state.nis = None;
        }
        for r in &mut race.racers.rivals {
            r.state.nis = None;
        }
        // The racers' sound contexts are new with the race (**inferred**: a
        // restart keeps their addresses, but a start scene's events replace
        // any pattern before it is heard).
        if let Some(a) = self.audio.as_mut() {
            a.ai_patterns.clear();
        }
        // 0x436840 case 0 (no other human racing, 0x5F0100): with Crowds
        // off no scene, and race status +0xC keeps 0x5FC740's 0.9 s (the
        // marks-based step of Crowds off, `shop_scene::crowds_off_marks`,
        // is case 3's, the multiplayer pick).
        // Drag races play one too (0x436840 case 0 has no race-type test;
        // **observed** under Wine, race112: a Quick Race drag opens on a
        // letterboxed scene with the starter, then the chase view and the
        // drag tach in neutral, no 3-2-1).
        if self.settings.custom[5] == 0 || race.test.is_some() || self.race_flow.game_mode == 8 {
            return None;
        }
        let table = IntroTable::load(&self.game_data.root).map_err(|e| eprintln!("NIS table: {e:#}")).ok()?;
        let racers = race.racers.rivals.len() as u32 + 1;
        let picks = table.race_candidates(
            track.number,
            !race.reversed,
            racers,
            race.grid_slot as u8 + 1,
            track.start_kind as u8,
        );
        // NIS_PickScene 0x436840 case 0 (0x436A26): Rand_Int(count) on the
        // shared generator once, after the load (the clone drew on a
        // copy xored with the track before, which never moved the stream).
        let rng = &mut self.rng;
        let forced = debug_env().intro_nis.clone().and_then(|v| v.parse::<u8>().ok());
        let pick = match forced {
            Some(n) => picks
                .iter()
                .copied()
                .find(|&k| IntroTable::scene_number(table.race_list[k].hash) == Some(n))
                .or_else(|| table.race_list.iter().position(|e| IntroTable::scene_number(e.hash) == Some(n)))?,
            None => *picks.get(rng.below(picks.len().max(1) as u32) as usize)?,
        };
        let entry = table.race_list[pick];
        let n = IntroTable::scene_number(entry.hash)?;
        let scene = Scene::load(&self.game_data.root, SceneKind::Intro(n))
            .map_err(|e| eprintln!("race scene IntroNis{n:02}: {e:#}"))
            .ok()?;
        let d = self.sim.drive.as_ref()?;
        let (at, heading) = (race.route.start_line?, race.route.start_heading?);
        let ground = Vec3::new(at.x, at.y, d.world.grid_height(at));
        // The camera track: the entry's variant (+0x04, handed out with the
        // hash by case 0; "Track %d" of the scene's group).
        let name = format!("Track {}", entry.variant);
        let camera = nfsu2::shop_scene::load_cameras(&self.game_data.root)
            .into_iter()
            .find(|g| g.key == scene.hash)?
            .tracks
            .into_iter()
            .find(|t| t.name == name);
        camera.as_ref()?;
        let origin = origin(ground, heading);
        // The scene's characters (0x4377F0's 0x37040 records, set up by
        // 0x437BE0): the starter and the crowd.
        self.scenes.crowd = nfsu2::game_fs::read(&self.game_data.root, &format!("NIS/Scene_{}_BundleB.bun", scene.kind.name()))
            .ok()
            .and_then(|b| {
                nfsu2::character::SceneCharacters::parse(&b).map_err(|e| eprintln!("scene characters: {e:#}")).ok()
            })
            .filter(|c| !c.instances.is_empty())
            .map(|set| SceneCrowd::new(std::sync::Arc::new(set), origin, scene.length, &d.world));
        // Its line (`audio::speech::nis::race_start`): the NIS load
        // 0x435BE0 asks NisAudio_Request (0x435CE8) for the scene with the
        // entry's variant; the player car's engine sound index picks the
        // +4 tracks (`d.car` as 0x4690B0(0)'s context +0x8C).
        let car_type = self.game_data.car_types.iter().position(|t| t.name == d.car.car_type.name).unwrap_or(0);
        let tuple = nfsu2::audio::speech::nis::race_start(
            n,
            entry.variant,
            self.race_flow.game_mode,
            nfsu2::audio::engine::sound_index(car_type),
            false,
        );
        if let Some(a) = self.audio.as_mut() {
            a.streams.play_nis(&a.mixer, nfsu2::audio::speech::nis::EVENT_START, &tuple, 2);
            // 0x475330 → 0x46CD20 as the line starts: the backup line
            // (scene, track, 4) armed (docs/native-fixes/N-AUD2.tsv; cleaned up by
            // `backup_update` in `stream_sounds`).
            a.streams.race_start_line_started();
        }
        Some(ShopRun {
            audio: nfsu2::shop_scene::audio::Run::with_carry(self.pacing.cadence.clock.carry),
            companion: None,
            car_slot: 1,
            scene,
            shop: None,
            origin,
            frame: 0.0,
            events: 0,
            key: 0,
            camera,
        })
    }

    /// The race's ending scene (**verified** unless marked). Every race
    /// flow set-up flags it (0x60A878: flow +0x44 = 1). Entering the
    /// post-race flow state 7 (0x603EC0 message 1; 0x60AAB0 posts it once
    /// the race is over) with the flag set and no replay running, the
    /// flow clears it and loads (+8 = 5, event 4 → LOADING): PC_Loading
    /// goes up (0x6039A6), the NIS manager is flagged (0x430430), and
    /// once the track streamer is idle (0x5D6170) 0x437200 picks and
    /// loads the scene — 0x436840 case 1, `IntroTable::ending_candidates`
    /// — and 0x43BAA0 starts it (0x43B8E0 → 0x43B000 → NIS_SceneStart
    /// 0x437910, the player's +0x234 its id). Its origin (0x43B000 type 1
    /// → 0x431990 with both flags clear): the marker 0x42FC60 gives with
    /// its flag clear — the start line on a circuit, the finish line point
    /// to point — at the ground, turned by the marker's heading + 0x4000
    /// twice (+180°, where a start scene's ∓0x4000 cancel). The camera is
    /// the scene's generic group, "Track %d" of the entry's variant
    /// (+0x04), the car `ZPM_Car1`, the characters the bundle's. The
    /// post-race screens (0x4A46E0 pushes the chain's first) stand over
    /// it. Observed under Wine (Quick Race, Resort Loop): "Winner!" over
    /// the race for 3 s, then PC_Loading, then Event Results over
    /// EndingNis04 (the crowd round the car on the start line).
    /// AI wins bind the winner and player to entry +6/+7. A human win's
    /// +7 names player 1 (0x432A14), not an AI runner-up; this game is single-player.
    /// Body-torque response and rev patterns remain unported.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn play_race_ending(
        &mut self,
        track: u16,
        at: Vec3,
        heading: f32,
        winner: &nfsu2::handling::Handling,
        player_won: bool,
        companion: Option<SceneCar>,
    ) {
        use nfsu2::shop_scene::{IntroTable, Scene, SceneKind, origin};
        self.scenes.shop = None;
        self.scenes.crowd = None;
        if self.sim.drive.is_none() || track == 0x1262 {
            return;
        }
        // A new scene's slots start at count 0 (0x43B172).
        if let Some(d) = self.sim.drive.as_mut() {
            d.state.nis = None;
        }
        // The post-race flow's own LOADING: the race's song goes on.
        self.begin_loading_in_flow();
        // The files read since the race started, when that is done
        // (`endings_ahead`): the same readings, not made again.
        let mut ahead = self.endings_ahead_take().unwrap_or_default();
        let mut crowd_ahead = None;
        let picked = (|| {
            let table = match ahead.table.take() {
                Some(t) => t,
                None => IntroTable::load(&self.game_data.root).map_err(|e| eprintln!("NIS table: {e:#}")).ok()?,
            };
            // 0x42FED0 reads the results winner's tuned rear split;
            // 0x436BC5 draws from the shared generator (no track reseed).
            let forced = debug_env().ending_nis.clone().and_then(|v| v.parse::<u8>().ok());
            let pick = match forced {
                Some(n) => table.ending_list.iter().position(|e| {
                    IntroTable::ending_number(e.hash) == Some(n)
                        && e.flags & 0x40 != 0
                        && (e.flags & 0x80 != 0) == player_won
                })?,
                None => table.pick_ending(track, winner, true, player_won, &mut self.rng)?,
            };
            let entry = table.ending_list[pick];
            let n = IntroTable::ending_number(entry.hash)?;
            let scene = match ahead.scenes.remove(&n) {
                Some((scene, crowd)) => {
                    crowd_ahead = Some(crowd);
                    scene
                }
                None => Scene::load(&self.game_data.root, SceneKind::Ending(n))
                    .map_err(|e| eprintln!("ending scene EndingNis{n:02}: {e:#}"))
                    .ok()?,
            };

            let name = format!("Track {}", entry.variant);
            let camera = ahead
                .cameras
                .take()
                .unwrap_or_else(|| nfsu2::shop_scene::load_cameras(&self.game_data.root))
                .into_iter()
                .find(|g| g.key == scene.hash)
                .and_then(|g| g.tracks.into_iter().find(|t| t.name == name));
            let companion = companion.filter(|_| !player_won).and_then(|mut car| {
                car.slot = usize::from(entry.rest[0]);
                // Do not fall back to Car1 when this scene has no bound channel.
                scene.racer(car.slot, 0.0)?;
                Some(car)
            });
            Some((scene, camera, entry.variant, usize::from(entry.rest[if player_won { 0 } else { 1 }]), companion))
        })();
        // Its line: the NIS load (0x435BE0 → 0x484510 kind 2) sets
        // NIS_Select_End by the scene and the adjusted variant's track bit
        // (0x484A98..0x484ADD).
        if let (Some((scene, _, variant, _, _)), Some(d)) = (picked.as_ref(), self.sim.drive.as_ref()) {
            let car_type = self.game_data.car_types.iter().position(|t| t.name == d.car.car_type.name).unwrap_or(0);
            let track =
                nfsu2::audio::speech::nis::track(*variant, self.race_flow.game_mode, nfsu2::audio::engine::sound_index(car_type));
            let name = scene.kind.name();
            self.scene_line(&name, track);
        }
        if let (Some((scene, camera, _, car_slot, companion)), Some(d)) = (picked, self.sim.drive.as_ref()) {
            let ground = Vec3::new(at.x, at.y, d.world.grid_height(at));
            let origin = origin(ground, heading + std::f32::consts::PI);
            let crowd = crowd_ahead.unwrap_or_else(|| super::endings_ahead::crowd_of(&self.game_data.root, &scene));
            self.scenes.crowd = crowd
                .map(|set| SceneCrowd::new(set, origin, scene.length, &d.world));
            self.scenes.shop = Some(ShopRun {
                audio: nfsu2::shop_scene::audio::Run::with_carry(self.pacing.cadence.clock.carry),
                companion,
                car_slot,
                scene,
                shop: None,
                origin,
                frame: 0.0,
                events: 0,
                key: 0,
                camera,
            });
            self.shop_scene_frame(0.0);
        }
        self.loads.loading = None;
    }
}

/// The in-engine scenes playing (were `Game` fields). Every field
/// starts empty (`Default`).
#[derive(Default)]
pub(crate) struct Scenes {
    /// The shop drive-in or drive-out scene playing (`nfsu2::shop_scene`).
    pub(crate) shop: Option<ShopRun>,
    /// A shop / dyno scene that ended in this frame's update: its audio
    /// update of that frame still runs (`engine_sound`; under Wine
    /// ShopExit1's last scripted update ran in the frame whose clock passed
    /// the scene's length).
    pub(crate) audio_tail: Option<ShopRun>,
    /// The race start scene's characters (`nfsu2::character`): the
    /// starter and the crowd, played on to the scene's end (header +0x28)
    /// after the green light ends the scene's camera.
    pub(crate) crowd: Option<SceneCrowd>,
}
