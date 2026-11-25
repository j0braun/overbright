use crate::*;

impl Game {
    /// The performance parts of the car driven, for its sounds (0x4593B0
    /// reads the ride's upgrade state): Rachel's D-Day preset or a Quick
    /// Race car's parts (`player_car`), else the profile's. Under Wine
    /// Rachel's 350Z plays engine entry 27 (level 2; the CAR event's
    /// parameter 0 = 27), not the stock entry 25 empty parts give.
    pub(crate) fn driven_perf(&self) -> std::collections::BTreeSet<u8> {
        if self.quick.car.is_some() || self.profile.as_ref().is_some_and(|p| p.car.is_empty()) {
            self.player_car().2
        } else {
            self.profile.as_ref().map(|p| p.perf.clone()).unwrap_or_default()
        }
    }

    /// A race's ending scene is playing (behind the post-race screens).
    pub(crate) fn ending_scene(&self) -> bool {
        self.scenes.shop.as_ref().is_some_and(|r| matches!(r.scene.kind, nfsu2::shop_scene::SceneKind::Ending(_)))
    }

    /// The race's AI racers while its ending plays, by rival index: their
    /// cars stay in the world with their sound contexts (under Wine,
    /// 2026-11-04, every racer's CAR instance stays alive behind Event
    /// Results and its context is updated, scripted idle); the winner
    /// drawn in the scene (`ShopRun::companion`) is heard where the scene
    /// puts it, the others where the race left them (the kept replay race,
    /// when there is one).
    pub(crate) fn ending_racers<'a>(
        shop_scene: &'a Option<ShopRun>,
        replay: &'a Option<Race>,
    ) -> Vec<(usize, &'a Car, &'a CarState)> {
        let Some(run) = shop_scene.as_ref().filter(|r| matches!(r.scene.kind, nfsu2::shop_scene::SceneKind::Ending(_)))
        else {
            return Vec::new();
        };
        let companion = run.companion.as_ref();
        let mut out: Vec<(usize, &Car, &CarState)> = match replay {
            Some(race) => race
                .racers.rivals
                .iter()
                .enumerate()
                .map(|(k, x)| match companion.filter(|c| c.rival == k) {
                    Some(c) => (k, &c.car, &c.state),
                    None => (k, &x.car, &x.state),
                })
                .collect(),
            None => Vec::new(),
        };
        if let Some(c) = companion.filter(|c| !out.iter().any(|o| o.0 == c.rival)) {
            out.push((c.rival, &c.car, &c.state));
        }
        out
    }

    pub(crate) fn reset_car_sound_context(&mut self) {
        if let Some(a) = self.audio.as_mut() {
            // Decoded GIN assets survive; the native car sound context does not.
            if let Some((entry, h, _)) = a.engine.as_mut() {
                *h = nfsu2::audio::hybrid::HybridMotor::new(*entry);
            }
            a.scripted_pattern = Default::default();
            a.ai_patterns.clear();
            a.scripted_engine = false;
        }
    }

    /// The player's engine: the recording the car and its engine parts
    /// pick, aimed at the drivetrain's rpm; silent off the road.
    /// The player engine (`audio::hybrid`): the HybridMotor chain gives
    /// the accel GIN's and the decel `_DCL` GIN's volume bytes and the
    /// recording rpm; both voices glide to it over 60 ms. The engine's
    /// EE bank "CAR" event plays the steady layers from the chain's
    /// per-frame block. level[0] is the mix map's (section 1; under Wine
    /// 18236 idle, 6477 while Rachel speaks: soundcmp).
    pub(crate) fn engine_sound(&mut self, dt: f32) {
        use nfsu2::audio::engine;
        self.mix_sounds(dt);
        let perf = self.driven_perf();
        // A race's ending scene plays behind the post-race screens with the
        // car's context scripted (observed under Wine: EndingNis02's pattern
        // 14 on the winning player's context, 30 updates a second).
        let ending = self.ending_scene();
        let driving = matches!(self.mode, Mode::Explore | Mode::Race(_)) || ending || self.tuning_menu_over_world();
        // A scene that ended in this frame's update still takes this
        // frame's audio update (`scene_audio_tail`).
        let mut tail = self.scenes.audio_tail.take();
        // The racers bound in an ending (`ending_racers`): its rival indices.
        let ending_rivals: Vec<usize> = if ending {
            Self::ending_racers(&self.scenes.shop, &self.sim.replay).iter().map(|r| r.0).collect()
        } else {
            Vec::new()
        };
        let Some(a) = self.audio.as_mut() else { return };
        let Some(d) = self.sim.drive.as_ref().filter(|_| driving) else {
            a.mixer.engine(0, 0.0, 0.0);
            a.mixer.engine(1, 0.0, 0.0);
            a.car_sfx.engine_silence(&mut a.mixer);
            return;
        };
        let car_type = self.game_data.car_types.iter().position(|t| t.name == d.car.car_type.name).unwrap_or(0);
        let entry = engine::entry(car_type, &perf);
        Self::engine_load(a, &self.game_data.root, entry);
        let Some((_, h, _)) = a.engine.as_mut() else { return };
        let input = Self::engine_input(d, &self.game_data.career, &perf, car_type);
        // CareerNis1 has no audio events, but its contexts are scripted
        // (observed under Wine, 2026-11-04: 403 scripted updates of the
        // player's context at 1000 rpm / 0 throttle, `docs/evidence/s3-audio`).
        let run = match self.scenes.shop.as_mut() {
            Some(r) => Some(r),
            None => tail.as_mut(),
        };
        Self::engine_scene_update(
            run,
            &self.mode,
            ending,
            &ending_rivals,
            h,
            (&mut a.scripted_engine, &mut a.scripted_pattern, &mut a.ai_patterns),
            &input,
            dt,
        );
        let roam_kind = self.sim.roam_camera.as_ref().filter(|_| matches!(self.mode, Mode::Explore)).map(|c| c.events.kind);
        Self::engine_post(a, d, &self.mode, roam_kind, dt);
    }

    /// The engine entry's recording loaded into the mixer's two engine voices
    /// when the car's entry changed (`engine_sound`).
    fn engine_load(a: &mut Audio, root: &std::path::Path, entry: usize) {
        use nfsu2::audio::{engine, gin, hybrid};
        if a.engine.as_ref().is_none_or(|e| e.0 != entry) {
            a.scripted_pattern = Default::default();
            a.scripted_engine = false;
            let load = |name: &str| {
                engine::gin_file(root, name).and_then(|p| nfsu2::game_fs::read_file(&p).ok()).and_then(|b| gin::Gin::parse(&b))
            };
            let h = hybrid::HybridMotor::new(entry);
            let accel = load(h.rec80().gin);
            let min = accel.as_ref().map_or(0.0, |g| g.min_rpm);
            let decel = if h.rec80().hybrid { load(h.rec84().gin) } else { None };
            a.mixer.set_engine(0, accel.map(gin::Granular::new));
            a.mixer.set_engine(1, decel.map(gin::Granular::new));
            a.engine = Some((entry, h, min));
        }
    }

    /// The HybridMotor's input for the player's car (`engine_sound`).
    fn engine_input(
        d: &Drive,
        data: &CareerData,
        perf: &std::collections::BTreeSet<u8>,
        car_type: usize,
    ) -> nfsu2::audio::hybrid::EngineInput {
        use nfsu2::audio::hybrid;
        let e = &d.car.car_type.handling.engine;
        // 0x484D1E: SFXCTL_Physics reads the input block's gas (car +0x30 →
        // +0x30 → +0x20C), not the engine's throttle +0x78 (0x5ABC00 zeroes
        // it under 0.15 and halves a light pedal under 3000 rpm).
        let throttle = d.state.gas();
        let gear = d.state.drivetrain.as_ref().map_or(1, |t| t.gear as i32);
        let up_rpm = d.state.drivetrain.as_ref().map_or(0.0, |t| t.shift_point_rpm(t.gear));
        let perf_level = |category: u8| {
            data
                .packages
                .iter()
                .filter(|p| p.category == category && p.parts.iter().any(|x| perf.contains(&x.id)))
                .map(|p| p.level as usize)
                .max()
                .unwrap_or(0)
                .min(3)
        };
        let transmission = perf_level(1);
        let input = hybrid::EngineInput {
            throttle,
            rpm: sound_rpm(&d.state, e.idle_rpm),
            idle_rpm: e.idle_rpm,
            max_rpm: e.max_rpm,
            gear,
            mph: d.state.speed() * 2.237,
            up_rpm,
            shift_record: nfsu2::audio::carfx::shift_record(car_type, transmission),
            transmission: transmission as i32,
            race_type_8: false,
            // 0x5ECD80: the player's car (+0x28 == 1) mid gear change.
            shifting: d.state.drivetrain.as_ref().is_some_and(|t| t.shifting()),
        };
        input
    }

    /// The chain's update for the frame: a NIS scene's scripted contexts,
    /// else the free-running engine (`engine_sound`). `scripted` is the
    /// audio's (`scripted_engine`, `scripted_pattern`, `ai_patterns`).
    #[allow(clippy::too_many_arguments)]
    fn engine_scene_update(
        run: Option<&mut ShopRun>,
        mode: &Mode,
        ending: bool,
        ending_rivals: &[usize],
        h: &mut nfsu2::audio::hybrid::HybridMotor,
        scripted: (&mut bool, &mut nfsu2::audio::scripted::Pattern, &mut Vec<nfsu2::audio::scripted::Pattern>),
        input: &nfsu2::audio::hybrid::EngineInput,
        dt: f32,
    ) {
        let (scripted_engine, scripted_pattern, ai_patterns) = scripted;
        if let Some(run) = run {
            // A NIS scene's CAR_AUDIO_PATTERN events and the scripted car
            // contexts (`shop_scene::audio`). The scene owns its event clock
            // and cursor; the car contexts' patterns and history survive it
            // (Exit -> Explore -> Entry). Scripted (Physics +0xD8): the player
            // (0x484C50) in every scene but a race start past its camera cut
            // (race status 2..4 with Physics +0xD0); the racers (0x47C490)
            // through the whole start scene. Audio updates every 1/30 s while
            // the scene holds the cars, else every 1/60 s (0x483B40).
            use nfsu2::shop_scene::{SceneKind, audio as nis};
            // The frame's cap is the one set before it (the camera cut's frame
            // still ran at 1/30 s); the player's scripting follows the race
            // status, which the cut changes within the frame (under Wine its
            // last scripted update is the last one before the cut).
            let holds = run.holds_cars();
            let capped = run.audio.held.replace(holds).unwrap_or(holds);
            let period =
                if capped { nfsu2::audio::scripted::SCENE_PERIOD } else { nfsu2::audio::scripted::FREE_PERIOD };
            let intro = matches!(run.scene.kind, SceneKind::Intro(_));
            let player_scripted = !intro || holds;
            // Slot binding (0x4328B0): a start scene's slot n is the racer on
            // grid slot n (0x432860); the other scenes bind the player to
            // their slot (0x4307F0; an ending's entry +6 / +7).
            // An ending binds the results' winner to slot 1 (0x430960; the
            // human to +6 / +7); the other racers' slots carry no events in
            // any shipped ending (**inferred** order, unused).
            let companion = run.companion.as_ref().map(|c| (c.rival, c.slot));
            let (player_slot, rivals): (usize, Vec<usize>) = match mode {
                Mode::Race(r) if intro => (r.grid_slot + 1, r.racers.rivals.iter().map(|x| x.slot + 1).collect()),
                _ if ending => (
                    run.car_slot,
                    ending_rivals.iter().map(|&k| companion.filter(|c| c.0 == k).map_or(0, |c| c.1)).collect(),
                ),
                _ => (run.car_slot, Vec::new()),
            };
            let racer_slots = ending_rivals.len().max(rivals.len());
            if ending {
                ai_patterns.resize_with(ending_rivals.iter().copied().max().map_or(0, |m| m + 1), Default::default);
            } else {
                ai_patterns.resize_with(racer_slots, Default::default);
            }
            *scripted_engine = player_scripted;
            let (pattern, ai) = (&mut *scripted_pattern, &mut *ai_patterns);
            // Observed under Wine (an ending lost to an AI, 2026-11-04): the
            // racers' contexts run scripted from the scene's second update
            // only, so the frame-0 event to the winner (EndingNis01's
            // pattern 13) is latched but cleared unstarted, and every racer
            // idles at 1000 rpm / 0 throttle, 30 updates a second.
            let mut update = run.audio.updates;
            run.audio.advance(&run.scene.audio_events, dt, period, |events| {
                nis::dispatch(events, player_slot, player_scripted, pattern);
                if player_scripted {
                    pattern.step();
                    h.update_scripted(&input, pattern, period);
                }
                if ending {
                    for (&k, &slot) in ending_rivals.iter().zip(&rivals) {
                        let p = &mut ai[k];
                        nis::dispatch(events, slot, update > 0, p);
                        if update > 0 {
                            p.step();
                        }
                    }
                } else {
                    for (p, &slot) in ai.iter_mut().zip(&rivals) {
                        nis::dispatch(events, slot, true, p);
                        p.step();
                    }
                }
                update += 1;
            });
            if !player_scripted {
                h.update(&input, dt);
            }
        } else {
            h.update(&input, dt);
        }
    }

    /// The chain's voices, send and pan posted to the mixer (`engine_sound`).
    fn engine_post(a: &mut Audio, d: &Drive, mode: &Mode, roam_kind: Option<u8>, dt: f32) {
        let Some((_, h, min)) = a.engine.as_mut() else { return };
        // CARSFX_PlayerEngine's slot 0 (0x473E60 / 0x474310): the level
        // and the azimuth the mix map gives it.
        let (level0, az) = (a.levels.level(0, 0), a.levels.az(0, 0));
        let (rpm, bytes) = h.voices(level0, *min);
        let (path, first, mut block) =
            (h.abk_path(), h.abk_first_block(), h.abk_block_fx(level0, a.reverb.out[2], a.reverb.out[3]));
        block[15] = az;
        // p21 = Engine +0x138 (0x46F460): the camera-distance low-pass.
        let far_cam = matches!(mode, Mode::Race(r) if matches!(r.camera.events.kind, 6 | 7))
            || matches!(roam_kind, Some(6 | 7));
        let cam_dist = (((d.camera.position.x - d.state.position.x) as f64).powi(2)
            + ((d.camera.position.y - d.state.position.y) as f64).powi(2))
        .sqrt() as i32;
        block[21] = nfsu2::audio::hybrid::HybridMotor::engine_cutoff(cam_dist, far_cam);
        a.car_sfx.engine_abk(&mut a.mixer, &path, &first, &block, dt);
        a.mixer.engine(0, rpm, bytes[0] as f32 / 127.0);
        a.mixer.engine(1, rpm, bytes[1] as f32 / 127.0);
        // The reverb send and dry bytes (0x473F0E / 0x473ECF; 0x4740E3 /
        // 0x474244 for the decel voice), from the reverb's wet A / dry A.
        let (send, dry) = a.reverb.engine_bytes(level0);
        a.mixer.engine_send(0, send, dry);
        a.mixer.engine_send(1, send, dry);
        a.mixer.engine_pan(0, az as u16);
        a.mixer.engine_pan(1, az as u16);
    }
}

/// The engine rpm the car's sound objects read (car +0x440): a car that
/// was never stepped (placed for a scene or the dyno screens) has no
/// drivetrain yet and idles (under Wine its context's rpm reads 1000 on
/// the dyno screen: the idle).
pub(crate) fn sound_rpm(state: &CarState, idle_rpm: f32) -> f32 {
    if state.drivetrain.is_some() { state.shown_rpm() } else { state.rpm().max(idle_rpm) }
}
