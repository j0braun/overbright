use crate::*;

impl Game {
    /// The mix map (`audio::mixmap`, 0x486920 / 0x478FE0): the file by the
    /// race (0x4869A0: drag 2, drift 1, else 0), the listener (the view
    /// camera and the player's car), the player, the AI racers and the
    /// traffic; the player's slots go to its sound objects this frame.
    pub(crate) fn mix_sounds(&mut self, dt: f32) {
        use nfsu2::audio::mixmap::{self, Scene};
        // A race's ending scene keeps the world's sounds running behind the
        // post-race screens (observed under Wine: the player's and every
        // racer's CAR, FX_ROADNOISE, FX_WIND, … instances stay alive), heard
        // from the scene's camera.
        let ending = self.ending_scene();
        let driving = matches!(self.mode, Mode::Explore | Mode::Race(_)) || ending || self.tuning_menu_over_world();
        let k = match &self.mode {
            Mode::Race(r) if r.drag => 2,
            Mode::Race(r) if r.scoring.drift.is_some() => 1,
            _ => 0,
        };
        let view_id = VIEW_ORDER[self.settings.players[0].camera.min(4) as usize] as u32;
        // The listener's view mode 0x82BB48 (0x46D7E0 at 0x46D961): 4
        // while a NIS scene's objects exist (0x827E98 / 0x827EB0 /
        // 0x827EC8) and the race status is not 3 or 4 (read under Wine:
        // 4 through DynoEnter, 0 on the dyno screen after it); a race
        // prov: inferred game.mix_sounds
        // start counts as racing past its camera cut (**inferred**: the
        // status change taken as the cut, `holds_cars`).
        let scene_view = self
            .scenes.shop
            .as_ref()
            .is_some_and(|r| !matches!(r.scene.kind, nfsu2::shop_scene::SceneKind::Intro(_)) || r.holds_cars());
        // The dyno's race status (0x890118, read under Wine with
        // race type 8): 2 through DynoEnter, 4 on the dyno screens after
        // it with the NIS objects gone. SFXObj_NISStream's f5 (+0x18,
        // 0x475330: race status state 2; section 0 class 5) feeds
        // MAPOUTPUT's event controls 3 / 6 / 10 (−720 mB on the song);
        // SFXCTL_MasterVol's f7 (+0x20, 0x471C50: race type 8, status 4, no
        // NIS objects) event control 4 (−240 mB). Other race types' race
        // prov: open game.mix_sounds-2
        // status is **open** (not observed), so both are given on the dyno
        // only.
        let on_dyno = self.dyno.tuning_session && self.on_dyno_track();
        let dyno_scene = on_dyno
            && self.scenes.shop.as_ref().is_some_and(|r| r.scene.kind == nfsu2::shop_scene::SceneKind::DynoEnter);
        let dyno_screen = on_dyno && self.scenes.shop.is_none();
        let root = self.game_data.root.clone();
        let Some(a) = self.audio.as_mut() else { return };
        if a.mix.as_ref().is_none_or(|m| m.0 != k) {
            a.mix = load_mix(&mut a.mix_failed, &root, k);
        }
        let st = &self.settings;
        // MasterVol f14's fill case (0x471B75): a drag or a drift, or no
        // EA Trax song for in game (playlist states 2 Racing / 3 All).
        let fill = k != 0 || !st.trax.iter().any(|&t| t == 2 || t == 3);
        let mut mv = mixmap::master_vol_fields(
            st.master,
            [st.menu_music, st.game_music, st.speech, st.effects, st.tires, st.engine],
            fill,
        );
        mv[7] = dyno_screen as i32;
        let player_vols =
            mixmap::player_vol_fields(st.master, st.speech, st.menu_music, st.effects, st.engine, st.tires, mv[14]);
        let master_vol = Some(mv);
        // SFXObj_Ambience's f2 (+0xC): the score's fade on a context
        // change (0x47B260), event control 1's trigger.
        let ambience = (0, 0, 0x22, a.streams.score_fading() as i32);
        // SFXObj_Speech's update (vtable 0x78B1A0 slot 9, 0x4751A0): both
        // fields 0 unless a line plays (0x45A550 on the speech stream) and
        // the NIS stream object's +0x4C is not 1; then f0 (+4) = 1 in the
        // front end (g_GameState 3), else f1 (+8) = 1, the other kept.
        // Section 0's event controls 2 / 5 / 7–9 (f1: −900, −300, −929,
        // −1320 mB) and 5 / 11 (f0: −689, −810), scaled by the speech
        // volume (MasterVol f10), duck the world under Rachel's lines:
        // under Wine the player engine's level 0 went 18236 → 6477 while
        // she spoke (soundcmp, 2026-11-01). `speech_playing` is 0x46D100(1):
        // 0x45A550 on the speech stream and the NIS stream object's +0x4C
        // clear (the backup stream; a NIS line itself does not count).
        if !a.streams.speech_playing(&a.mixer) {
            a.speech_fields = [0, 0];
        } else if driving {
            a.speech_fields[1] = 1;
        } else {
            a.speech_fields[0] = 1;
        }
        let speech = [(0, 0, 0x10, a.speech_fields[0]), (0, 0, 0x11, a.speech_fields[1])];
        let Some(d) = self.sim.drive.as_ref().filter(|_| driving) else {
            a.levels = Default::default();
            a.impacts.clear();
            a.impact_copies.clear();
            // The global section runs in the front end too (0x478FE0 each
            // frame): the music objects' levels.
            let scene = Scene { master_vol, sfx: vec![ambience, speech[0], speech[1]], ..Default::default() };
            a.music_levels = a.mix.as_mut().map(|(_, m)| {
                m.frame(&scene, dt);
                [m.level(0, 0, 0, 0), m.level(0, 0, 0, 1), m.level(0, 0, 2, 0), m.level(0, 0, 2, 1)]
            });
            a.speech_levels = a.mix.as_ref().map(|(_, m)| [m.level(0, 0, 1, 0), m.level(0, 0, 1, 1)]);
            a.fe_levels = a.mix.as_ref().map_or([0x7FFF; 8], |(_, m)| std::array::from_fn(|s| m.level(0, 0, 6, s)));
            return;
        };
        let m_in = MixIn {
            view_id,
            scene_view,
            dyno_scene,
            ending,
            master_vol,
            player_vols,
            ambience,
            speech,
        };
        let (scene, scrape_points, impacts) = Self::mix_scene(
            a,
            d,
            &self.mode,
            &self.settings,
            &self.scenes.shop,
            &self.sim.replay,
            self.sim.traffic.as_ref(),
            &m_in,
            dt,
        );
        Self::mix_levels(a, d, &scene, &scrape_points, &impacts, dt);
    }

    /// The frame's mix-map scene: listener, player, racers, traffic, scrapes
    /// and impacts (`mix_sounds`); the scrape points and the impacts taken go
    /// back for `mix_levels`.
    #[allow(clippy::too_many_arguments, clippy::type_complexity)]
    fn mix_scene(
        a: &mut Audio,
        d: &Drive,
        mode: &Mode,
        settings: &options::Settings,
        shop_scene: &Option<ShopRun>,
        replay: &Option<Race>,
        free_traffic: Option<&Traffic>,
        m_in: &MixIn,
        dt: f32,
    ) -> (nfsu2::audio::mixmap::Scene, Vec<(u64, [f32; 2])>, Vec<nfsu2::audio::collision::Impact>) {
        use nfsu2::audio::mixmap::{self, Listener, Scene};
        let MixIn { view_id, scene_view, dyno_scene, ending, master_vol, player_vols, ambience, speech } = *m_in;
        a.mix_slew = mixmap::view_slew(a.mix_slew, view_id);
        let mut scene = Scene {
            listener: Some(Listener {
                cam: xy(d.camera.position),
                cam_dir: dir(d.camera.forward()),
                car: xy(d.state.position),
                car_dir: dir(d.state.forward()),
            }),
            view: if scene_view { 4 } else { mixmap::view_mode(view_id) },
            player: Some(car_fields(&d.car, &d.state, a.mix_slew, a.car_sfx.skid_level())),
            master_vol,
            player_vols,
            tunnel: a.driveby.tunnel.fields(),
            ..Default::default()
        };
        scene.sfx.push(ambience);
        scene.sfx.extend(speech);
        scene.sfx.push((0, 0, 0x55, dyno_scene as i32));
        if let Mode::Race(r) = mode {
            // Each racer's SFXCTL_Skid output is its copy's skid field.
            let skids = a.car_sfx.ai_skid_levels();
            scene.ai = r
                .racers
                .rivals
                .iter()
                .enumerate()
                .map(|(k, x)| car_fields(&x.car, &x.state, 0, skids.get(k).copied().unwrap_or(0)))
                .collect();
            // SFXCTL_MasterVol f6 (0x471C0B): a replay paused (manager
            // +0x14, also set by every skip, 0x4A4C8C) fades the world's
            // sounds out through event control 0 (`Scene::sound_hold`).
            scene.sound_hold = r.replay.playback.as_ref().is_some_and(|p| p.paused || p.skip.is_some());
        } else if ending {
            scene.ai =
                Self::ending_racers(shop_scene, replay).iter().map(|x| car_fields(x.1, x.2, 0, 0)).collect();
        }
        // The traffic: the race's, or free roam's (none for an ending: the
        // race's traffic is not kept, **inferred** silent).
        let traffic = match mode {
            Mode::Race(r) => r.racers.traffic.as_ref().filter(|t| !t.paused),
            _ if ending => None,
            _ => free_traffic,
        };
        // Section 3's copies are the traffic manager's contexts (0x485FA0:
        // one per instance), each at its attached car; a free context's
        // objects are silent, its copy sits at the listener's car.
        if let Some(t) = traffic {
            let here = xy(d.state.position);
            scene.traffic = a
                .others
                .traffic_contexts()
                .iter()
                .map(|k| k.and_then(|k| t.cars.get(k)).map_or(here, |c| xy(c.state.position)))
                .collect();
        }
        scene.enviro = a.enviro.mix_positions();
        // Section 5: one SFXObj_Collision copy per live collision object at
        // its scrape point (`Scrapes::points`).
        let scrape_points = a.scrapes.points();
        scene.collisions = scrape_points.iter().map(|e| e.1).collect();
        // The impacts' sound objects (one copy each, after the scrapes').
        let impacts = std::mem::take(&mut a.impacts);
        scene.collisions.extend(impacts.iter().map(|i| xy(i.at)));
        // Their first pass's fields (0x461B60), then the earlier impacts'
        // copies until their envelopes are over (0.3 s: the events' 267 ms).
        let em = settings.effects * settings.master;
        for (j, i) in impacts.iter().enumerate() {
            for (f, v) in nfsu2::audio::collision::mix_fields(i, d.state.position, em) {
                scene.sfx.push((5, scrape_points.len() + j, f, v));
            }
        }
        // A started object's copy stays while its layers or splash play
        // prov: inferred audio.collision.copy_life
        // (**inferred**: natively the copy lives with its sound
        // object, whose end with its record is not traced).
        let (mixer, env) = (&a.mixer, &a.env);
        a.impact_copies.retain_mut(|c| {
            c.1 -= dt;
            c.1 > 0.0
                || c.2.as_ref().is_some_and(|l| {
                    l.voices.iter().any(|v| mixer.playing(v.0)) || l.splash.is_some_and(|id| env.splash_alive(id))
                })
        });
        // At most 16 copies: the oldest without a playing object go first.
        while a.impact_copies.len() + impacts.len() > 16 {
            let Some(k) = a.impact_copies.iter().position(|c| c.2.is_none()) else { break };
            a.impact_copies.remove(k);
        }
        scene.collisions.extend(a.impact_copies.iter().map(|c| c.0));
        a.impact_copies.extend(impacts.iter().map(|i| (xy(i.at), 0.3, None)));
        // The sound objects' own fields from their last frame.
        scene.sfx.push((1, 0, 0x30, a.car_sfx.nitrous_field()));
        scene.sfx.push((1, 0, 0xB0, a.env.bottom_out_field()));
        // CARSFX_Rain f2 (0x461890).
        scene.sfx.push((1, 0, 0x92, a.env.rain.field()));
        for k in 0..scene.traffic.len() {
            let (woosh, engine) = a.others.traffic_fields(k);
            scene.sfx.push((3, k, 0x12, woosh));
            scene.sfx.push((3, k, 0x03, engine));
        }
        (scene, scrape_points, impacts)
    }

    /// The mix map's frame and the levels it gives the sound objects, with the
    /// impacts' start volumes (`mix_sounds`).
    fn mix_levels(
        a: &mut Audio,
        d: &Drive,
        scene: &nfsu2::audio::mixmap::Scene,
        scrape_points: &[(u64, [f32; 2])],
        impacts: &[nfsu2::audio::collision::Impact],
        dt: f32,
    ) {
        if let Some((_, m)) = a.mix.as_mut() {
            m.frame(scene, dt);
            // NFSU2_MIX_TRACE="s.copy.class,…": those master channels'
            // inputs once a second (a check aid).
            if let Some(t) = debug_env().mix_trace.clone() {
                a.reverb_debug.0 += 1;
                if a.reverb_debug.0 % 60 == 0 {
                    for w in t.split(',') {
                        let v: Vec<usize> = w.split('.').filter_map(|x| x.parse().ok()).collect();
                        if let [s, c, k] = v[..] {
                            eprint!("mix {s}.{c}.{k}: {}", m.trace(s, c, k as u32));
                        }
                    }
                }
            }
            a.music_levels = Some([m.level(0, 0, 0, 0), m.level(0, 0, 0, 1), m.level(0, 0, 2, 0), m.level(0, 0, 2, 1)]);
            a.speech_levels = Some([m.level(0, 0, 1, 0), m.level(0, 0, 1, 1)]);
            a.fe_levels = std::array::from_fn(|s| m.level(0, 0, 6, s));
            // FX_Scrape's level and azimuth: SFXObj_Collision's slot 9
            // (0x461CE0 → the sound object's +0xD4 / +0xDC).
            let slots: Vec<(u64, Option<u32>)> =
                scrape_points.iter().enumerate().map(|(i, e)| (e.0, m.packed(5, i, 0, 9))).collect();
            a.scrapes.set_mix(&slots);
            // The started objects' level each frame (0x461CE0 → +0xC0, then
            // 0x47BB30 → 0x46CF70 for the layers and 0x481FD0 for a splash):
            // the copies of earlier frames sit after this frame's impacts'.
            let sp = scrape_points.len();
            let old = a.impact_copies.len().saturating_sub(impacts.len());
            for (k, c) in a.impact_copies.iter_mut().enumerate() {
                let Some(l) = c.2.as_mut() else { continue };
                let copy = if k < old { sp + impacts.len() + k } else { sp + (k - old) };
                let v = m.level(5, copy, 0, l.slot);
                if debug_env().audio_debug {
                    eprintln!("audio: impact object {k} V {v}");
                }
                if l.start > 0 {
                    // Linear in V: the layer's own volume × V >> 15 (the
                    // integer steps are below 1/32767 of the gain).
                    let f = v as f32 / l.start as f32;
                    for &(id, g) in &l.voices {
                        a.mixer.set_gain(id, g * f);
                    }
                }
                if let Some(id) = l.splash {
                    a.env.splash_start(id, v);
                }
            }
            // An impact's start volume: its copy's slot for its kind
            // (0x461CE0: +0x124 1..8 → slot 1..8, else 0; slot 0x38).
            // The queue's flush (0x481850): at most five objects play.
            if let Some(c) = a.collision.as_mut() {
                // (impact, start volume, layers); a water drum's object is
                // queued for its splash even without a layer.
                let objects: Vec<(usize, i32, Vec<nfsu2::audio::collision::Voice>)> = impacts
                    .iter()
                    .enumerate()
                    .map(|(j, i)| {
                        let slot = if (1..=8).contains(&i.kind) { i.kind } else { 0 };
                        let start = m.level(5, sp + j, 0, slot);
                        let voices = c.play(i, start);
                        if debug_env().audio_debug {
                            let layers: Vec<String> =
                                voices.iter().map(|v| format!("{}@{}", (v.1 * 32767.0) as i32, v.2)).collect();
                            eprintln!(
                                "audio: impact kind {} volume {} start {start} at {:.1} m layers {}",
                                i.kind,
                                i.volume,
                                i.at.distance(d.state.position),
                                layers.join(" ")
                            );
                        }
                        (j, start, voices)
                    })
                    .filter(|o| !o.2.is_empty() || impacts[o.0].splash)
                    .collect();
                let queued = objects.iter().map(|o| (impacts[o.0].at.distance(d.state.position), o.2.clone())).collect();
                let mixer = &a.mixer;
                let mut handles = Vec::new();
                nfsu2::audio::mixer::stems::set("collision");
                let started =
                    a.impact_pool.flush_started(queued, &mut |id| mixer.playing(id), &mut |id| mixer.stop(id), &mut |v| {
                        let h = mixer.play_handle_after(&v.0, v.1, v.2, v.3);
                        handles.push(h);
                        h
                    });
                nfsu2::audio::mixer::stems::clear();
                // The started objects in start order, each its layers'
                // handles; a drum's splash rides on the start (0x4818A6 →
                // 0x46AE00) at the object's +0xC0.
                let mut handles = handles.into_iter();
                for k in started {
                    let (j, start, voices) = &objects[k];
                    let i = &impacts[*j];
                    let ids: Vec<(u32, f32)> = voices.iter().map(|v| (handles.next().unwrap_or(0), v.1)).collect();
                    let splash = i.splash.then(|| a.env.splash(&mut a.mixer, *start));
                    if i.splash && debug_env().audio_debug {
                        eprintln!("audio: water drum splash, start {start}");
                    }
                    if let Some(c) = a.impact_copies.get_mut(old + j) {
                        c.2 = Some(crate::audio::ImpactLive {
                            slot: if (1..=8).contains(&i.kind) { i.kind } else { 0 },
                            start: *start,
                            voices: ids,
                            splash,
                        });
                    }
                }
            }
            a.levels = m.levels(1, 0);
            a.ai_levels = (0..scene.ai.len()).map(|k| m.levels(2, k)).collect();
            a.traffic_levels = (0..scene.traffic.len()).map(|k| m.levels(3, k)).collect();
            a.enviro_levels = [4, 7, 8].map(|s| (0..m.copies(s)).map(|k| m.levels(s, k)).collect());
        } else {
            a.levels = Default::default();
            a.music_levels = None;
            a.speech_levels = None;
            a.fe_levels = [0x7FFF; 8];
            a.ai_levels = vec![Default::default(); scene.ai.len()];
            a.traffic_levels = vec![Default::default(); scene.traffic.len()];
            a.enviro_levels = scene.enviro.clone().map(|v| vec![Default::default(); v.len()]);
        }
    }
}

/// A point's ground-plane coordinates (`mix_scene`).
fn xy(v: Vec3) -> [f32; 2] {
    [v.x, v.y]
}

/// A direction's unit ground-plane vector, east for none (`mix_scene`).
fn dir(v: Vec3) -> [f32; 2] {
    let l = (v.x * v.x + v.y * v.y).sqrt();
    if l == 0.0 { [1.0, 0.0] } else { [v.x / l, v.y / l] }
}

/// A car's mix-map fields (`mix_scene`).
fn car_fields(car: &Car, st: &CarState, slew: i32, skid: i32) -> nfsu2::audio::mixmap::CarFields {
    use nfsu2::audio::mixmap::CarFields;
        // SFXCTL_3DRightWheelPos / LeftWheelPos (0x100B0 / 0x100C0)
        // point at SFXCTL_Wheel's +0xD0 / +0xC0, which its update
        // (0x46EA50 → 0x46E8B0) sets each frame to the car's position
        // (+0x60) + 1.5 (0x7ED268) × its forward row (+0x90) ∓ 0.8
        // (0x7ED264) × its left row (+0xA0): two points 1.5 m ahead,
        // 0.8 m to each side (read under Wine: 28° = azimuth 5112 /
        // 60423 at idle).
        let _ = car;
        let side = |s: f32| {
            let p = st.position + st.orientation * Vec3::X * 1.5 + st.orientation * Vec3::Y * (0.8 * s);
            [p.x, p.y]
        };
        let e = &car.car_type.handling.engine;
        let ctx_rpm =
            (sound_rpm(st, e.idle_rpm) - e.idle_rpm) / (e.max_rpm - e.idle_rpm).max(1.0) * 9000.0 + 1000.0;
        CarFields {
            pos: xy(st.position),
            right: side(-1.0),
            left: side(1.0),
            speed: st.speed(),
            ctx_rpm,
            airborne: st.wheels_down() < 4,
            skid,
            view_slew: slew,
        }
}

/// The mix map for race kind `k` (0x486920 loads it once per race kind and
/// plays without it when it is missing), unless it failed for `k` before:
/// the frame asks every frame, so a missing or broken file is read once
/// until the race kind changes (`FailedLoads`, one kept).
pub(crate) fn load_mix(
    failed: &mut nfsu2::failed_load::FailedLoads<usize>,
    root: &std::path::Path,
    k: usize,
) -> Option<(usize, nfsu2::audio::mixmap::Mix)> {
    use nfsu2::audio::mixmap::{Mix, MixMap};
    failed.load(k, "mix map", || {
        MixMap::load(root, k)
            .map(|m| (k, Mix::new(m)))
            .ok_or_else(|| format!("SOUND/MixMaps/{} missing or unreadable", MixMap::file_name(k)))
    })
}

#[cfg(test)]
mod failed_mix_tests {
    /// A missing mix map is read once across the frames, and
    /// again once the race kind changes.
    #[test]
    fn a_missing_mix_map_is_read_once_until_the_race_kind_changes() {
        let mut failed = nfsu2::failed_load::FailedLoads::new(1);
        let root = std::path::Path::new("/nonexistent/ob-failed-mix-map");
        for _ in 0..120 {
            assert!(super::load_mix(&mut failed, root, 0).is_none());
        }
        assert_eq!(failed.attempts(), 1, "one read across 120 frames");
        for _ in 0..120 {
            assert!(super::load_mix(&mut failed, root, 2).is_none());
        }
        assert_eq!(failed.attempts(), 2, "the drag's map read once");
        assert!(super::load_mix(&mut failed, root, 0).is_none());
        assert_eq!(failed.attempts(), 3, "back to kind 0: read again once");
    }
}


/// The values `mix_sounds` works out before the scene (`mix_scene`).
struct MixIn {
    view_id: u32,
    scene_view: bool,
    dyno_scene: bool,
    ending: bool,
    master_vol: Option<[i32; 16]>,
    player_vols: [i32; 7],
    ambience: (usize, usize, u32, i32),
    speech: [(usize, usize, u32, i32); 2],
}
