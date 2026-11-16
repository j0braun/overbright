use crate::*;

impl Game {
    /// The impacts the physics posted (`audio::collision`): those within
    /// 100 m of the player play, each stitch record's layers through the
    /// mixer. 0x5F1170 takes the nearest of the listener object's
    /// (0x890080) positions +0x80 / +0x90 (and +0xA0 / +0xB0, +0xC0 /
    /// +0xD0 with more players); the clone's listener is the player's car
    /// (inferred: which of the two is the car).
    pub(crate) fn impact_sounds(&mut self) {
        let impacts = self.sim.contact_sounds.impacts.drain();
        let pushes = self.sim.contact_sounds.pushes.drain();
        let Some(a) = self.audio.as_mut() else { return };
        let Some(d) = self.sim.drive.as_ref().filter(|_| matches!(self.mode, Mode::Explore | Mode::Race(_))) else {
            // The showcase keeps FX_Misc running for its camera shutter
            // (`Scrapes::shutter`).
            if matches!(self.mode, Mode::Menu(Menu::Showcase(_))) && self.sim.drive.is_some() {
                a.scrapes.update(&mut a.mixer, &[], self.pacing.frame_dt);
            } else {
                a.scrapes.silence(&mut a.mixer);
            }
            a.hydraulics.silence(&mut a.mixer);
            return;
        };
        // CARSFX_Hydraulics (0x47DD40, 0x4615E0): the corners' states, the
        // frame's kicks and the mix map's class 8 slot 0.
        let hy = &d.state.hydraulics;
        a.hydraulics.update(&mut a.mixer, hy.state, hy.kicked, a.levels.level(8, 0), self.pacing.frame_dt);
        // The pushes within 100 m of the listener arm their scrapes
        // (0x5A98CB → 0x4792D0).
        let pushes: Vec<_> = pushes.into_iter().filter(|p| p.at.distance(d.state.position) < 100.0).collect();
        a.scrapes.update(&mut a.mixer, &pushes, self.pacing.frame_dt);
        // Each impact's sound object gets its collision copy in the next
        // mix pass, which gives its start volume (`mix_sounds`); with no
        // mix map every level is full.
        let near = impacts.into_iter().filter(|i| i.at.distance(d.state.position) < 100.0);
        if a.mix.is_some() {
            a.impacts.extend(near);
            a.impacts.truncate(64);
        } else if let Some(c) = a.collision.as_mut() {
            for i in near {
                for (s, gain, pitch, delay) in c.play(&i, 0x7FFF) {
                    a.mixer.play_after(&s, gain, pitch, delay);
                }
                if i.splash {
                    a.env.splash(&mut a.mixer, 0x7FFF);
                }
            }
        }
    }

    /// The player car's inputs to its effect sounds
    /// (`audio::carfx::CarInput`): the shot's stage, speed, gear and rpm,
    /// the nitrous level and the shift record. The sound context's levels
    /// (0x4593B0: round(upgrade state float[cat]) clamped 0–3, cat 2
    /// nitrous, 1 transmission) are taken as the highest package level of
    /// the category with a part fitted (**inferred**: the floats' writer is
    /// not traced).
    pub(crate) fn car_sfx_input(&self) -> Option<nfsu2::audio::carfx::CarInput> {
        let d = self.sim.drive.as_ref()?;
        // A car placed for a scene and never stepped (the dyno's, a
        // showcase's) has no drivetrain yet; its sound objects still run
        // (under Wine in DynoEnter the CAR, FX_ROADNOISE, FX_WIND, FX_SKID,
        // FX_TURBO_01 and CAR_Sputter instances are alive and SFXCTL_Tunnel
        // keeps the reverb's dry level at 0x7FFF; docs/engine-level.md):
        // no shot, neutral, no throttle, as `engine_sound` takes it.
        let t = d.state.drivetrain.as_ref();
        let car_type = self.game_data.car_types.iter().position(|t| t.name == d.car.car_type.name).unwrap_or(0);
        let perf = self.driven_perf();
        let level = |category: u8| {
            self.game_data.career
                .packages
                .iter()
                .filter(|p| p.category == category && p.parts.iter().any(|x| perf.contains(&x.id)))
                .map(|p| p.level as usize)
                .max()
                .unwrap_or(0)
                .min(3)
        };
        Some(nfsu2::audio::carfx::CarInput {
            stage: t.map_or(0, |t| t.nitrous),
            speed: d.state.speed(),
            gear: t.map_or(1, |t| t.gear),
            rpm: sound_rpm(&d.state, d.car.car_type.handling.engine.idle_rpm),
            nitrous_level: level(2),
            shift_record: nfsu2::audio::carfx::shift_record(car_type, level(1)),
            // The sound context's +0x54: the pedal (0x484E75), not the
            // drivetrain's throttle (they differ in shift cuts and launches).
            throttle: d.state.gas(),
            gas: d.state.gas(),
            // Turbo level: round(upgrade state float[4]) (0x45EB30).
            turbo_record: nfsu2::audio::carfx::turbo_record(car_type, level(4)),
            turbo: d.car.car_type.handling.engine.curve_334.values.iter().any(|&v| v > 0.0),
            sound_index: nfsu2::audio::engine::sound_index(car_type),
            drift: matches!(&self.mode, Mode::Race(r) if r.scoring.drift.is_some()),
            bumper_cam: self.settings.players[0].camera == 0,
            shift: Default::default(),
            mix: Default::default(),
            skid: skid_input(&d.state),
        })
    }

    /// The option volumes, the EA Trax songs and the song captions
    /// (`audio::streams`): front-end songs in the menus (the menu mask),
    /// racing songs while driving, none during movies or a cut scene.
    pub(crate) fn stream_sounds(&mut self, dt: f32) {
        // Leaving the list (0xC98356BA with +0x114 set: 0x458C40, +0x116
        // = 0) ends the purge test.
        if self.shop.purge_test && !self.purge_list_open() {
            self.purge_test_set(false);
        }
        let context = self.music_context();
        let hold_speech = self.speech_held();
        let in_game = matches!(context, MusicContext::Game | MusicContext::Dyno | MusicContext::Score)
            || (self.sim.drive.is_some() && !matches!(context, MusicContext::FrontEnd | MusicContext::ScoreFrontEnd));
        let st = &self.settings;
        let music = if in_game { st.game_music } else { st.menu_music };
        let levels = nfsu2::audio::streams::Streams::levels(st.master, st.effects, st.engine, st.speech, music);
        let (trax, random) = (st.trax_playlist(), st.trax_random);
        // The score's music context (0x46B330): the front end's 0x1C; in
        // game field 0 (an int) of the "Ambience" parameter map (0x82C710)
        // at the player car's x, y (the car object's +0x60 / +0x64,
        // prov: inferred game.stream_sounds
        // **inferred** as its position); with no map it stays as it was.
        let zone = if in_game {
            self.sim.drive.as_ref().and_then(|d| {
                let map = d.world.parameter_maps.get(&nfsu2::formats::hash::bin_hash("Ambience"))?;
                let v = map.sample(d.state.position.truncate()).first()?.to_bits() as i32;
                u8::try_from(v).ok()
            })
        } else {
            // In a career, by the shop visited (0x46B330).
            let career = self.profile.is_some() && self.save_flow.customize.is_none() && !self.race_flow.quick_free_run;
            let shop = self
                .shop.visit
                .and_then(|h| self.game_data.career.shops.iter().find(|s| s.hash == h))
                .map(|s| (s.scene_file.as_str(), s.hash));
            Some(nfsu2::audio::music::front_end_context(career, shop))
        };
        // The "Volumes" map's field 0 at the player car (0x46B330 → +0x60,
        // when the map is bound, 0x82BEAC).
        let volumes_mb = if in_game {
            self.sim.drive.as_ref().and_then(|d| {
                let map = d.world.parameter_maps.get(&nfsu2::formats::hash::bin_hash("Volumes"))?;
                Some(map.sample(d.state.position.truncate()).first()?.to_bits() as i32)
            })
        } else {
            None
        };
        let fe_levels = self.fe_event_levels();
        // The race status object (0x890118) for the NIS stream object's
        // backup cleanups (0x475330 → 0x4753E5): its state and its clock
        // +0x14; None outside a race.
        let race_status = match &self.mode {
            Mode::Race(r) => {
                Some(r.status(self.scenes.shop.as_ref().is_some_and(|run| run.frame < run.scene.marks[1] as f32)))
            }
            _ => None,
        };
        let Some(a) = self.audio.as_mut() else { return };
        // The FE sounds' events (the FE_MB player's update 0x4793D0 and the
        // library ticks). The garage's turn rate (0x7EC858 = |car +0x248|,
        // the right stick's) is 0: the clone's FE car has no stick turn.
        a.fe.update(&mut a.mixer, &fe_levels, 0.0, dt);
        // The FE_COMMON player's update (0x4815B0): its stitch slot at
        // SFXObj_FEHUD's slot 4.
        a.part_stitches.update(&mut a.mixer, fe_levels[4], dt);
        // With the mix map's music levels the option volumes are in them
        // (SFXCTL_MasterVol's fields), so the music bus stays at 1 — in
        // the front end too (slot 0: the menu music level through master
        // 0, the score through master 4; read from the original, sound5).
        let music_levels = a.music_levels;
        // The mix map's levels already hold the effects, engine and master
        // volumes (SFXCTL_MasterVol's fields into section 0's controls, the
        // player's volume fields into section 1, 0x45C210), and the
        // library scales a voice by its volume byte alone (0x72412A: byte ×
        // 1/127 into the voice's +0x38; SndMix_Render adds no option
        // volume): with the map loaded the effects and engine buses stay
        // at 1 (under Wine with the effects volume 0 the engine's abk
        // layers still sound; docs/engine-level.md).
        let levels =
            if a.mix.is_some() { nfsu2::audio::mixer::Levels { effects: 1.0, engine: 1.0, ..levels } } else { levels };
        let levels = if music_levels.is_some() {
            nfsu2::audio::mixer::Levels { music: 1.0, ..levels }
        } else if context == MusicContext::ScoreFrontEnd {
            nfsu2::audio::mixer::Levels { music: st.effects * st.master, ..levels }
        } else {
            levels
        };
        let zone = zone.or_else(|| a.streams.score().map(|s| s.0)).unwrap_or(nfsu2::audio::music::SCORE_FRONT_END);
        // NFSU2_AUDIO_DEBUG=1: every 2 s, the volumes, what plays and the
        // output's peak since the last line (to catch a mix going quiet).
        if debug_env().audio_debug {
            static LAST: std::sync::Mutex<Option<Instant>> = std::sync::Mutex::new(None);
            let mut last = LAST.lock().unwrap();
            if last.is_none_or(|t| t.elapsed().as_secs_f32() >= 2.0) {
                *last = Some(Instant::now());
                let (peak, state) = a.mixer.take_peak();
                eprintln!(
                    "audio: peak {peak:.4} | master {} effects {} engine {} speech {} music {music} ({context:?}, score {:?} zone {zone}) -> {levels:?} | {state}",
                    st.master,
                    st.effects,
                    st.engine,
                    st.speech,
                    a.streams.score()
                );
            }
        }
        // The speech stream's volume each frame (0x46CB48): kind 3's level
        // (0x45AC20: the speech object's vtable +0x38, slot 0 when the
        // audio system's +0x90 is 1 — the front end — else 1) → trunc(level
        // / 32768 · 100), × 1/127 (0x46CB88): at most 100/127. The speech
        // volume is inside the level (control 2), so the speech bus is 1;
        // the NIS line's level holds the effects volume (section 0 control
        // 3 in each of class 5's masters).
        // The NIS stream's (0x46CBA1 and the same at 0x46CB48 for its
        // current request): 0x45AC20(kind) for a kind other than 3 reads
        // SFXObj_NISStream (section 0 class 5, the stream object's +0x50,
        // set by 0x4752D0) at the slot the kind picks (jump table 0x45AD18:
        // 1 → 2, 2 / 7 / 9 → 0, 4 → 4, 5 / 8 → 3, 6 → 1; 0 for an unbound
        // slot), then the same trunc(level / 32768 · 100) / 127.
        let nis_slot = match a.streams.nis_kind {
            1 => Some(2),
            2 | 7 | 9 => Some(0),
            4 => Some(4),
            5 | 8 => Some(3),
            6 => Some(1),
            _ => None,
        };
        let nis_level = a.mix.as_ref().zip(nis_slot).map_or(0, |((_, m), slot)| m.level(0, 0, 5, slot));
        let nis_vol = (nis_level as f32 / 32768.0 * 100.0).trunc() / 127.0;
        let levels = if let Some(sl) = a.speech_levels {
            let vol = (sl[usize::from(in_game)] as f32 / 32768.0 * 100.0).trunc() / 127.0;
            a.streams.set_speech_gains(&a.mixer, vol, nis_vol);
            nfsu2::audio::mixer::Levels { speech: 1.0, ..levels }
        } else {
            levels
        };
        a.mixer.set_levels(levels);
        a.streams.hold_speech(&a.mixer, hold_speech);
        // The NIS stream object's update after its kind-2 part (0x475330):
        // the button-through start of the backup line and its cleanups.
        a.streams.backup_update(&a.mixer, race_status);
        if let Some(c) = a.streams.music_frame(&a.mixer, context, &trax, random, zone, music_levels, volumes_mb, dt) {
            self.platform.chyron = Some(nfsu2::audio::music::Chyron::new(c.song, self.now(), self.real_now()));
        }
    }

    /// The nitrous and gear-shift sounds while driving.
    pub(crate) fn car_sounds(&mut self, dt: f32) {
        let CarSoundsIn {
            input,
            mut env,
            others,
            mut ai_skids,
            driveby,
            drag,
            soundfx_in,
            enviro_in,
            reverb_region,
            root,
            ending_ai,
            ending_input,
            ending_env,
            listener,
        } = self.car_sounds_in();
        let Some(a) = self.audio.as_mut() else { return };
        // The horns' player inputs (car +0x90, +0x42C; 0x89E7D8).
        if let Some(c) = driveby.as_ref() {
            a.others.player_forward = c.forward;
            a.others.player_speed = c.forward_speed;
        }
        a.others.drag = drag;
        match input {
            Some(mut input) => {
                // The engine chain (run first, `engine_sound`) hands over
                // its SFXCTL_Shifting / SFXCTL_Engine outputs.
                if let Some((_, h, _)) = a.engine.as_ref() {
                    input.shift = h.shift_signal();
                    // The effects read the sound context (+0x54, the
                    // throttle input %): in a scene the script's (Physics
                    // +0xD8, 0x46FA30), as under Wine in DynoEnter, where
                    // the turbo blows off on the pattern's lifts.
                    if a.scripted_engine {
                        input.throttle = h.ctx.input / 100.0;
                    }
                }
                input.mix = a.levels;
                for (k, x) in ai_skids.iter_mut().enumerate() {
                    x.mix = a.ai_levels.get(k).copied().unwrap_or_default();
                }
                a.car_sfx.set_ai_skids(ai_skids);
                a.car_sfx.update(&mut a.mixer, input, dt);
                Self::car_soundfx(a, &soundfx_in);
                Self::car_env(a, &mut env, dt);
                Self::car_others(a, &others, dt);
                Self::car_enviro(a, &enviro_in, dt);
                Self::car_driveby(a, driveby, self.sim.drive.as_ref(), &root, reverb_region, dt);
            }
            None => Self::car_idle(a, ending_input, ending_env, &ending_ai, listener, dt),
        }
    }

    /// What `car_sounds` reads of the game: the player's inputs (silent unless
    /// driving), the world's, the ending scene's racers and car.
    fn car_sounds_in(&self) -> CarSoundsIn {
        // The tuning session's menus over the dyno track keep the car's
        // sound objects running (under Wine on the Performance Tuning
        // screen after DynoEnter: the CAR, road, wind, skid, turbo and
        // sputter instances alive, the engine idling at about −40 dBFS).
        let driving = matches!(self.mode, Mode::Explore | Mode::Race(_)) || self.tuning_menu_over_world();
        let input = if driving { self.car_sfx_input() } else { None };
        let (env, others) = match self.world_sfx_input().filter(|_| driving) {
            Some((e, o)) => (Some(e), o),
            None => (None, Default::default()),
        };
        // CARSFX_DriveBy's car fields (car +0x60, +0x90, +0x42C, +0x430).
        let driveby = self.sim.drive.as_ref().filter(|_| driving).map(|d| nfsu2::audio::driveby::CarIn {
            position: d.state.position,
            forward: d.state.forward(),
            forward_speed: d.state.speed(),
            speed: d.state.velocity.length(),
        });
        let drag = matches!(&self.mode, Mode::Race(r) if r.drag);
        // The SoundFX streams' inputs: the race flow's state
        // (`Game::race_flow_state`, verified under Wine), the nitrous
        // fraction (0x5925C0), the engine blown (0x5ECE10).
        let soundfx_in = SoundFxIn {
            flow: self.race_flow_state(),
            nitrous: self
                .sim.drive
                .as_ref()
                .and_then(|d| d.state.nitrous)
                .filter(|t| t.size > 0)
                .map_or(0.0, |t| t.fraction()),
            blown: self.sim.drive.as_ref().and_then(|d| d.state.drivetrain.as_ref()).is_some_and(|t| t.blown()),
        };
        // The world animations' entities (0x439F40) and the player's car.
        let enviro_in =
            self.sim.drive.as_ref().filter(|_| driving).map(|d| (d.world.anim_points(), vec![d.state.position]));
        // The "Reverb" parameter map's field 0 (the region's preset) and
        // field 2 (a level offset, mB) at the car (0x47C6D0).
        let reverb_region = self.sim.drive.as_ref().filter(|_| driving).and_then(|d| {
            let map = d.world.parameter_maps.get(&nfsu2::formats::hash::bin_hash("Reverb"))?;
            let row = map.sample(d.state.position.truncate());
            Some((row.first()?.to_bits() as i32, row.get(2).map_or(0, |v| v.to_bits() as i32)))
        });
        let root = self.game_data.root.clone();
        // The racers' engines while a race's ending plays (`ending_racers`):
        // each at its stock engine entry, its context's rpm / throttle (the
        // scripted idle the ending gives them), heard from the player's car.
        let ending_ai: Vec<(usize, nfsu2::audio::traffic::AiInput)> = if driving {
            Vec::new()
        } else {
            Self::ending_racers(&self.scenes.shop, &self.sim.replay)
                .into_iter()
                .map(|(k, car, st)| {
                    let car_type = self.game_data.car_types.iter().position(|t| t.name == car.car_type.name).unwrap_or(0);
                    (
                        k,
                        nfsu2::audio::traffic::AiInput {
                            entry: nfsu2::audio::engine::entry(car_type, &Default::default()),
                            ctx_rpm: 1000.0,
                            throttle: 0.0,
                            position: st.position,
                            mix: Default::default(),
                        },
                    )
                })
                .collect()
        };
        // A race's ending scene behind the post-race screens: the player's
        // own sound objects keep running from the scene car (**observed**
        // under Wine, a won EndingNis01: FX_WIND's level and pitch,
        // FX_ROADNOISE's pitch and FX_SKID's slip levels follow the car
        // through its drift; docs/evidence/ending-hold-audio/), until the
        // scene's last frame, where FX_SKID's and FX_WIND's levels drop to
        // 0 within 0.1 s (the scene's 550 frames; what lowers them is **not
        // traced**, the clone silences them there). The SFXCTL updates run
        // for the player's objects whatever the screen (0x4735E0 tests
        // only the owner kind, **verified**).
        let ending_car = !driving
            && self.scenes.shop.as_ref().is_some_and(|r| {
                matches!(r.scene.kind, nfsu2::shop_scene::SceneKind::Ending(_)) && r.frame < r.scene.length
            });
        let ending_input = if ending_car { self.car_sfx_input() } else { None };
        let ending_env = if ending_car { self.world_sfx_input().map(|(e, _)| e) } else { None };
        let listener = self.sim.drive.as_ref().map_or((Vec3::ZERO, Vec3::ZERO), |d| (d.state.position, d.state.velocity));
        // The AI racers' skids: one CARSFX_Skids per racer whose sound
        // context runs (the racers `world_sfx_input` hands over).
        let ai_skids = match &self.mode {
            Mode::Race(r) if !others.0.is_empty() => r
                .racers
                .rivals
                .iter()
                .take(others.0.len())
                .map(|x| {
                    let car_type = self.game_data.car_types.iter().position(|t| t.name == x.car.car_type.name).unwrap_or(0);
                    nfsu2::audio::carfx::AiSkidInput {
                        skid: skid_input(&x.state),
                        sound_index: nfsu2::audio::engine::sound_index(car_type),
                        mix: Default::default(),
                    }
                })
                .collect(),
            _ => Vec::new(),
        };
        CarSoundsIn {
            input,
            env,
            others,
            ai_skids,
            driveby,
            drag,
            soundfx_in,
            enviro_in,
            reverb_region,
            root,
            ending_ai,
            ending_input,
            ending_env,
            listener,
        }
    }

    /// The named SoundFX streams' asks (`car_sounds`).
    fn car_soundfx(a: &mut Audio, soundfx_in: &SoundFxIn) {
        // The named SoundFX streams (sound6): GB_NOS from
        // CARSFX_Nitrous's start (0x484F57..0x4850A1), BLOWNENGINE
        // from SFXCTL_Physics (0x484E82..0x484F11), CollisionStream
        // from the crash camera (0x486130).
        let busy = a.streams.nis_busy(&a.mixer);
        let mut asks: Vec<SoundFx> = Vec::new();
        match a.car_sfx.nos_edge {
            Some(true) if soundfx_in.nitrous > 1.5 && soundfx_in.flow == 4 && !busy => {
                asks.push(SoundFx::GbNos)
            }
            // The shot's end (state 1 → 0, 0x484F57..0x485005,
            // **verified** in the disassembly): for the player's
            // audio object (+0x14 = 1, the kind BLOWNENGINE's gate
            // reads) 0x82BB29 = 0, the NIS stream ([0x82BA30] +8)
            // stopped at once (0x7227CB on its handle), its queue
            // flushed (0x482970(1)) and a pending request freed
            // (0x575640). GB_NOS (record 13 of SoundFX_Select, an
            // 89.5 s stream) lasts only while the shot runs; the
            // clone let it play out (a player report).
            Some(false) => {
                a.soundfx.gb_nos = false;
                a.streams.stop_nis(&a.mixer);
            }
            _ => {}
        }
        if !a.soundfx.blown_asked && soundfx_in.blown && !matches!(soundfx_in.flow, 2 | 3 | 7) && !busy {
            a.soundfx.blown_asked = true;
            asks.push(SoundFx::BlownEngine);
        }
        if std::mem::take(&mut a.soundfx.crash_camera) {
            asks.push(SoundFx::Collision);
        }
        for ask in asks {
            soundfx_request(a, ask, soundfx_in.flow);
        }
    }

    /// The player's road / wind / rain inputs (`car_sounds`).
    fn car_env(a: &mut Audio, env: &mut Option<nfsu2::audio::roadwind::EnvInput>, dt: f32) {
        if let Some(e) = env.as_mut() {
            e.wheel = a.car_sfx.wheel_ctl().cloned();
            e.mix = a.levels;
            e.stream_busy = a.streams.nis_busy(&a.mixer);
            a.env.update(&mut a.mixer, e, dt);
            // CARSFX_Rain; its thunder is the THUNDERSTRM NIS line
            // (stream kind 4, SoundFX_Select = (4, variant)), its
            // callback (0x461880) taken as the line's end.
            if a.env.rain.thunder_playing && !e.stream_busy {
                a.env.rain.thunder_done();
            }
            if let Some(v) = a.env.rain_update(&mut a.mixer, e, dt) {
                // 0x4846F0: no thunder while GB_NOS plays (0x82BB29);
                // the request clears 0x82BB2B (view mode not 3).
                a.soundfx.collision = false;
                let played = !a.soundfx.gb_nos
                    && a.streams.play_nis(
                        &a.mixer,
                        nfsu2::audio::speech::nis::EVENT_SOUNDFX,
                        &[4, v as u32],
                        4,
                    );
                if debug_env().audio_debug {
                    eprintln!(
                        "audio: thunder variant {v} (rain {:.2}) {}",
                        e.rain,
                        if played { "plays" } else { "fits no line" }
                    );
                }
                if !played {
                    a.env.rain.thunder_done();
                }
            }
        }
    }

    /// The AI racers' and traffic cars' sounds (`car_sounds`).
    fn car_others(
        a: &mut Audio,
        others: &(Vec<nfsu2::audio::traffic::AiInput>, Vec<nfsu2::audio::traffic::TrafficInput>, (Vec3, Vec3), usize),
        dt: f32,
    ) {
        let ai: Vec<_> = others
            .0
            .iter()
            .enumerate()
            .map(|(k, x)| nfsu2::audio::traffic::AiInput {
                mix: a.ai_levels.get(k).copied().unwrap_or_default(),
                ..*x
            })
            .collect();
        // The traffic contexts take section 3's copies in order.
        a.others.traffic_levels = a.traffic_levels.clone();
        a.others.traffic_cars = others.3;
        a.others.update(&mut a.mixer, &ai, &others.1, others.2, dt);
        if a.others.stereos > 0 && debug_env().audio_debug {
            eprintln!("audio: {} traffic car stereo(s) started", a.others.stereos);
        }
    }

    /// The world animations' sounds (`car_sounds`).
    fn car_enviro(
        a: &mut Audio,
        enviro_in: &Option<(Vec<nfsu2::audio::enviro::AnimPoint>, Vec<Vec3>)>,
        dt: f32,
    ) {
        if let Some((points, players)) = enviro_in.as_ref() {
            let setup = a.enviro_setup;
            a.enviro.set_up(&mut a.mixer, setup);
            let [f, p, t] = &a.enviro_levels;
            let input = nfsu2::audio::enviro::Input {
                points,
                players,
                listener: players[0],
                mix: [f, p, t],
                url: setup.url,
            };
            a.enviro.update(&mut a.mixer, &input, dt);
            if !a.enviro.log.is_empty() && debug_env().audio_debug {
                let [f, p, t] = a.enviro.playing();
                for (name, e, d, level) in &a.enviro.log {
                    eprintln!(
                        "audio: {name} {e} starts at {d:.0} m, level {level} (playing: fountains {f}, planes {p}, trains {t})"
                    );
                }
            }
        }
    }

    /// The drive-by, tunnel and reverb sounds (`car_sounds`).
    fn car_driveby(
        a: &mut Audio,
        driveby: Option<nfsu2::audio::driveby::CarIn>,
        drive: Option<&Drive>,
        root: &std::path::Path,
        reverb_region: Option<(i32, i32)>,
        dt: f32,
    ) {
        if let Some(car) = driveby {
            // The loaded track's zones (L4RG on the dyno).
            if let Some(d) = drive {
                a.driveby.tunnel.set_region(&root, &d.region);
            }
            let levels = a.levels;
            a.driveby.update(&mut a.mixer, &car, |slot| (levels.level(12, slot), levels.az(12, slot)), dt);
            // SFXObj_Reverb's start in game (0x4632C0: Hills, then
            // 0x45DB90(8, 0)) and SFXCTL_Tunnel's zones and region
            // (0x483437..0x4834A3); a fade's end reloads the graph
            // (0x4633F0).
            if a.reverb_loaded.is_none() {
                a.reverb = Default::default();
                a.reverb_loaded = Some(8);
                a.fx_rate = a.mixer.rate();
                a.mixer.set_fx(nfsu2::audio::reverb::Graph::load(&root, 8, a.mixer.rate()));
                a.reverb.request(8, 0);
            }
            let t = &a.driveby.tunnel;
            a.reverb.frame(t.inside.then_some(t.inside_kind), reverb_region, dt);
            if debug_env().audio_debug && {
                a.reverb_debug.0 += 1;
                a.reverb_debug.0 % 15 == 0
            } {
                eprintln!(
                    "audio: reverb zone {:?} region {:?} preset {} wet A {} dry A {} (fade {}), engine send / dry {:?}, wet peak {:.0}",
                    t.inside.then_some(t.inside_kind),
                    reverb_region,
                    a.reverb.current,
                    a.reverb.out[0],
                    a.reverb.out[1],
                    a.reverb.fading(),
                    a.reverb.engine_bytes(a.levels.level(0, 0)),
                    a.mixer.take_fx_peak()
                );
            }
            // A device opened at another rate than the graph's
            // (the mix starts at 48 kHz, `Mix::set_rate`): reload.
            if a.reverb.reload || a.fx_rate != a.mixer.rate() {
                a.reverb_loaded = Some(a.reverb.current);
                a.fx_rate = a.mixer.rate();
                a.mixer.set_fx(nfsu2::audio::reverb::Graph::load(&root, a.reverb.current, a.mixer.rate()));
                if debug_env().audio_debug {
                    eprintln!(
                        "audio: reverb preset {} ({})",
                        a.reverb.current,
                        nfsu2::audio::reverb::PRESETS[a.reverb.current]
                    );
                }
            }
        }
    }

    /// Not driving: the player's objects silenced, or the ending scene's car
    /// and racers kept running (`car_sounds`).
    fn car_idle(
        a: &mut Audio,
        ending_input: Option<nfsu2::audio::carfx::CarInput>,
        ending_env: Option<nfsu2::audio::roadwind::EnvInput>,
        ending_ai: &[(usize, nfsu2::audio::traffic::AiInput)],
        listener: (Vec3, Vec3),
        dt: f32,
    ) {
        if a.reverb_loaded.take().is_some() {
            a.mixer.set_fx(None);
        }
        a.driveby.silence(&mut a.mixer);
        match ending_input {
            Some(mut input) => {
                if let Some((_, h, _)) = a.engine.as_ref() {
                    input.shift = h.shift_signal();
                    if a.scripted_engine {
                        input.throttle = h.ctx.input / 100.0;
                    }
                }
                input.mix = a.levels;
                a.car_sfx.update(&mut a.mixer, input, dt);
                match ending_env {
                    Some(mut e) => {
                        e.wheel = a.car_sfx.wheel_ctl().cloned();
                        e.mix = a.levels;
                        e.stream_busy = a.streams.nis_busy(&a.mixer);
                        a.env.update(&mut a.mixer, &e, dt);
                    }
                    None => a.env.silence(&mut a.mixer),
                }
            }
            None => {
                a.car_sfx.silence(&mut a.mixer);
                a.env.silence(&mut a.mixer);
            }
        }
        if ending_ai.is_empty() {
            a.others.silence(&mut a.mixer);
        } else {
            let ai: Vec<_> = ending_ai
                .iter()
                .enumerate()
                .map(|(i, (k, x))| {
                    let p = a.ai_patterns.get(*k);
                    nfsu2::audio::traffic::AiInput {
                        mix: a.ai_levels.get(i).copied().unwrap_or_default(),
                        ctx_rpm: p.map_or(1000.0, |p| p.rpm),
                        throttle: p.map_or(0.0, |p| p.throttle / 100.0),
                        ..*x
                    }
                })
                .collect();
            a.others.traffic_levels = Vec::new();
            a.others.traffic_cars = 0;
            a.others.update(&mut a.mixer, &ai, &[], listener, dt);
        }
        a.enviro.silence(&mut a.mixer);
    }

    /// The player's road / wind / bottom-out inputs (`audio::roadwind`)
    /// and the other cars' (`audio::traffic`): the AI racers (stock
    /// engine entry, the sound context's rpm and throttle), the traffic
    /// cars in the mix map's copy order, the player's position and
    /// velocity.
    #[allow(clippy::type_complexity)]
    pub(crate) fn world_sfx_input(
        &self,
    ) -> Option<(
        nfsu2::audio::roadwind::EnvInput,
        (Vec<nfsu2::audio::traffic::AiInput>, Vec<nfsu2::audio::traffic::TrafficInput>, (Vec3, Vec3), usize),
    )> {
        let d = self.sim.drive.as_ref()?;
        let s = &d.state;
        let susp = &d.car.car_type.handling.suspension;
        let mut compression = [0.0; 4];
        for (k, w) in s.wheels.iter().enumerate() {
            let travel = susp[(k >= 2) as usize].max_compression;
            compression[k] = if travel > 0.0 { w.compression / travel } else { 0.0 };
        }
        let env = nfsu2::audio::roadwind::EnvInput {
            // Car +0x430 is |velocity|, not the forward speed (+0x42C):
            // the NIS mover's export 0x5A2B70 stores sqrt(v · v) of the
            // body's velocity (+0xA0) and, beside it, forward · v; under
            // Wine (EndingNis03 on mode 5 and the braking before it on
            // mode 1) +0x430 read |body +0xA0| to 4 decimals (4.95 forward
            // vs 5.46 in the scene's slide). RoadNoise and WindNoise read it.
            speed: s.velocity.length(),
            wheel: None,
            down: [s.wheels[0].on_ground, s.wheels[1].on_ground, s.wheels[2].on_ground, s.wheels[3].on_ground],
            up_z: (s.orientation * Vec3::Z).z,
            compression,
            mix: Default::default(),
            event_kind: match &self.mode {
                Mode::Race(r) => r.camera.events.kind,
                Mode::Explore => self.sim.roam_camera.as_ref().map_or(0, |c| c.events.kind),
                _ => 0,
            },
            // The weather object's +0x1F0 (CARSFX_Rain, 0x474770).
            rain: d.weather.amount,
            stream_busy: false,
        };
        let mut ai = Vec::new();
        let mut traffic = Vec::new();
        let mut pool = 0;
        if let Mode::Race(r) = &self.mode {
            // During a race start scene the racers' contexts are scripted
            // (0x47C490 → 0x46FA30): rpm / throttle are their patterns'.
            let scripted = self
                .scenes.shop
                .as_ref()
                .is_some_and(|run| matches!(run.scene.kind, nfsu2::shop_scene::SceneKind::Intro(_)));
            for (k, x) in r.racers.rivals.iter().enumerate() {
                let car_type = self.game_data.car_types.iter().position(|t| t.name == x.car.car_type.name).unwrap_or(0);
                let e = &x.car.car_type.handling.engine;
                let pattern = self.audio.as_ref().and_then(|a| a.ai_patterns.get(k)).filter(|_| scripted);
                let ctx_rpm = pattern.map_or(
                    (x.state.shown_rpm() - e.idle_rpm) / (e.max_rpm - e.idle_rpm).max(1.0) * 9000.0 + 1000.0,
                    |p| p.rpm,
                );
                // The context's throttle is the driver's gas (car +0x30 →
                // +0x20C, SFXCTL_Physics 0x484C50), not the engine's: on
                // the MeloMvr the drivetrain is not run (round 13).
                let throttle = pattern.map_or(x.controls.gas, |p| p.throttle / 100.0);
                ai.push(nfsu2::audio::traffic::AiInput {
                    entry: nfsu2::audio::engine::entry(car_type, &Default::default()),
                    ctx_rpm,
                    throttle,
                    position: x.state.position,
                    mix: Default::default(),
                });
            }
        }
        let cars = match &self.mode {
            Mode::Race(r) => r.racers.traffic.as_ref().filter(|t| !t.paused),
            _ => self.sim.traffic.as_ref(),
        };
        if let Some(t) = cars {
            pool = t.cars.len();
            let event_kind = match &self.mode {
                Mode::Race(r) => r.camera.events.kind,
                Mode::Explore => self.sim.roam_camera.as_ref().map_or(0, |c| c.events.kind),
                _ => 0,
            };
            traffic = t
                .cars
                .iter()
                .enumerate()
                .filter(|(_, c)| c.lane.is_some())
                .map(|(key, c)| {
                    let w = &c.state.wheels;
                    let taxi = matches!(c.car.name.as_str(), "TAXI" | "TAXI02");
                    nfsu2::audio::traffic::TrafficInput {
                        position: c.state.position,
                        velocity: c.state.velocity,
                        mix: Default::default(),
                        down: [w[0].on_ground, w[1].on_ground, w[2].on_ground, w[3].on_ground],
                        up_z: (c.state.orientation * Vec3::Z).z,
                        event_kind,
                        key,
                        taxi,
                    }
                })
                .collect();
        }
        Some((env, (ai, traffic, (s.position, s.velocity), pool)))
    }
}


/// What `car_sounds` takes of the game state for the frame.
struct CarSoundsIn {
    input: Option<nfsu2::audio::carfx::CarInput>,
    env: Option<nfsu2::audio::roadwind::EnvInput>,
    others: (Vec<nfsu2::audio::traffic::AiInput>, Vec<nfsu2::audio::traffic::TrafficInput>, (Vec3, Vec3), usize),
    ai_skids: Vec<nfsu2::audio::carfx::AiSkidInput>,
    driveby: Option<nfsu2::audio::driveby::CarIn>,
    drag: bool,
    soundfx_in: SoundFxIn,
    enviro_in: Option<(Vec<nfsu2::audio::enviro::AnimPoint>, Vec<Vec3>)>,
    reverb_region: Option<(i32, i32)>,
    root: std::path::PathBuf,
    ending_ai: Vec<(usize, nfsu2::audio::traffic::AiInput)>,
    ending_input: Option<nfsu2::audio::carfx::CarInput>,
    ending_env: Option<nfsu2::audio::roadwind::EnvInput>,
    listener: (Vec3, Vec3),
}
/// The skid controllers' inputs from a car (the player's or an AI racer's:
/// SFXCTL_Wheel / SFXCTL_Skid read the same car fields, 0x46EA50).
pub(crate) fn skid_input(s: &nfsu2::drive::CarState) -> nfsu2::audio::skids::SkidInput {
    let mut skid = nfsu2::audio::skids::SkidInput {
        speed: s.speed(),
        slip_angle: s.slip_degrees().to_radians(),
        steer_degrees: s.steer_degrees,
        brake: s.brake_input,
        // The input block's +0x210 as the wheels' handbrake input (+0x17C): **inferred**.
        handbrake: s.wheels.iter().map(|w| w.handbrake).fold(0.0, f32::max),
        ..Default::default()
    };
    let v = s.velocity;
    let (f, up) = (s.forward(), s.orientation * Vec3::Z);
    let left = up.cross(f);
    skid.forward = v.dot(f);
    skid.local_dir = if v.length() > 0.0 { (v.dot(f) / v.length(), v.dot(left) / v.length()) } else { (1.0, 0.0) };
    for (o, w) in skid.wheels.iter_mut().zip(&s.wheels) {
        *o = nfsu2::audio::skids::WheelIn {
            on_ground: w.on_ground,
            surface: w.surface,
            slip: w.exported_slip,
            lateral: w.lateral_slip,
            spin_slip: w.slip,
        };
    }
    skid
}
