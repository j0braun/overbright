use crate::*;

impl Game {
    /// A key pressed (`exit` ends the game: Back on the main menu).
    pub(crate) fn key(&mut self, code: KeyCode, exit: &mut dyn FnMut()) -> Result<()> {
        let key = match code {
            KeyCode::ArrowLeft => Some(Key::Left),
            KeyCode::ArrowRight => Some(Key::Right),
            KeyCode::ArrowUp => Some(Key::Up),
            KeyCode::ArrowDown => Some(Key::Down),
            // The FE keyboard map: the first row per JOY id of the descriptor
            // table 0x7FDD10, its key an entry of the DIACTION table 0x7FD7E0.
            // FENG_SELECT / _ALTERNATE are Enter / Num Enter (rows 11, 12),
            // FENG_CANCEL is Esc only (row 13), FENG_LTRIGGER / _RTRIGGER are
            // [ / ] ("Menu L1" / "Menu R1", rows 9, 10).
            KeyCode::Enter | KeyCode::NumpadEnter => Some(Key::Accept),
            KeyCode::Escape => Some(Key::Back),
            KeyCode::BracketLeft => Some(Key::LTrigger),
            KeyCode::BracketRight => Some(Key::RTrigger),
            // $JOY_EVENT_FENG_BUTTON1$ (Cancel Changes, Change Color,
            // Remove All). The PC key is C: the descriptor row for JOY 0x20
            // (0x7FDDD0) reads keyboard uAppData 0x23 "Menu Customise" (DIK
            // 0x2E, active low) through 0x571C80; NumEnter is Select Alternate.
            KeyCode::KeyC => Some(Key::Button1),
            // The rest of the FE keyboard map's first rows: FENG_START (row 4)
            // and FENG_BUTTON2 (row 7) both read comma; the clone sends only
            // Start, as no clone screen reacts to Button2 (formats.md).
            // FENG_BUTTON0 P (row 5), FENG_BUTTON3 period (row 8), FENG_QUIT Q (row 24).
            KeyCode::Comma => Some(Key::Start),
            KeyCode::KeyP => Some(Key::Button0),
            KeyCode::Period => Some(Key::Button3),
            KeyCode::KeyQ => Some(Key::Quit),
            _ => None,
        };
        // Alt+Enter toggles the Screen Mode anywhere (clone option).
        if matches!(code, KeyCode::Enter | KeyCode::NumpadEnter)
            && (self.platform.keys.contains(&KeyCode::AltLeft) || self.platform.keys.contains(&KeyCode::AltRight))
        {
            self.settings.fullscreen = !self.settings.fullscreen;
            let _ = self.settings.save();
            return Ok(());
        }
        // F3 cycles the performance overlay's layouts anywhere (hidden,
        // Minimal, Standard, Full); Shift+F3 hides it (docs/perf-overlay.md).
        if code == KeyCode::F3 {
            self.platform.perf_overlay
                .cycle(self.platform.keys.contains(&KeyCode::ShiftLeft) || self.platform.keys.contains(&KeyCode::ShiftRight));
            return Ok(());
        }
        // The Enhancements page's Mark Spot Key writes a report of this
        // spot (off by default; docs/mark-spot.md).
        if self.settings.enh.extras.mark_spot != 0 && mark::key_index(code) == self.settings.enh.extras.mark_spot {
            self.mark_spot();
            return Ok(());
        }
        // Ctrl+R reloads every texture pack, with Reload Packs When
        // Changed on (docs/pack-hot-reload.md).
        if code == KeyCode::KeyR
            && nfsu2::qol::reload_packs()
            && (self.platform.keys.contains(&KeyCode::ControlLeft) || self.platform.keys.contains(&KeyCode::ControlRight))
        {
            nfsu2::render::pack_reload::request_full();
            return Ok(());
        }
        // Photo mode takes every other key while it is up.
        if self.photo.is_some() {
            return self.photo_key(code);
        }
        // The Controls screen waiting for a key binds the next one pressed
        // to the selected action's column (0x5B9130's capture).
        if let Mode::Menu(Menu::Controls(c)) = &mut self.mode
            && c.capturing
        {
            // A controller's capture: Esc cancels it (0x87075C, 0x5CA730),
            // other keys do nothing.
            if c.device != 0 {
                if code == KeyCode::Escape {
                    c.capturing = false;
                    if let Some(d) = self.platform.pads.devices.get_mut(c.device - 1) {
                        d.map.capture = 0;
                    }
                }
                return Ok(());
            }
            c.capturing = false;
            c.bound = true;
            if let Some(name) = key_name(code) {
                self.settings.bind(c.selected, c.column, name);
                let _ = self.settings.save();
            }
            return Ok(());
        }
        let acts: [bool; 18] = std::array::from_fn(|row| self.is_action(code, row));
        self.dispatch(Some(code), key, acts, exit)
    }

    /// A key's or a controller's press past the screens' own handling:
    /// `key` the FE key it gives, `acts` the Controls rows it is bound to
    /// (a controller's: the actions its record maps the input to,
    /// `pad_frame`).
    pub(crate) fn dispatch(
        &mut self,
        code: Option<KeyCode>,
        key: Option<Key>,
        acts: [bool; 18],
        _exit: &mut dyn FnMut(),
    ) -> Result<()> {
        use nfsu2::frontend::options::action;
        let act = |row: usize| acts[row];
        let (pause, camera, reset, up, down) = (
            act(action::PAUSE),
            act(action::CAMERA),
            act(action::RESET),
            act(action::SHIFT_UP),
            act(action::SHIFT_DOWN),
        );
        let engage = act(action::ENGAGE);
        let world_map = act(action::WORLD_MAP);
        let read_sms = act(action::READ_SMS);
        let career_stats = act(action::CAREER_STATS);
        let bound = Acts { pause, camera, reset, up, down, engage, world_map, read_sms, career_stats };
        match &self.mode {
            // Back (Escape) on UI_Main does nothing, as in the original
            // (seen under Wine, twice; user decision 2026-10-31): Q's quit
            // question and closing the window end the game.
            Mode::Menu(Menu::Icons(_, MenuKind::Main)) if key == Some(Key::Back) => {}
            // 'Q' / 'q' (0x558B80, the window's character): on a screen
            // with a help bar (0x52CF00's record +0x1C nonzero) once the
            // splash was passed (0x836494), in the front end (0x8654A4 = 3)
            // "Are you sure you want to quit?" (0x9DE6B485; Quit Game
            // 0x1C101778 → 0x919B1DD9, Cancel 0x541389E5 → 0x42CA53FC, also
            // Back; the package by the size rule, 0x783FE5) over the screen,
            // Cancel focused (seen under Wine on UI_Main).
            Mode::Menu(m)
                if key == Some(Key::Quit)
                    && !matches!(m, Menu::Dialog(..))
                    && self.help_screen().is_some_and(|(mask, _)| mask != 0) =>
            {
                self.quit_question()
            }
            Mode::Menu(_) => {
                if let Some(k) = key {
                    // A loading pause: draw nothing new until it is done.
                    self.menu_key(k)?;
                    self.track_undim();
                }
            }
            // Agreed deviation (docs/native-deviations.md "Skipping scenes"): Enter skips an in-engine scene
            // (the career start fly-over, a shop's drive-in / drive-out) to
            // its end, as if it had played out.
            Mode::Explore
                if self.scenes.shop.is_some() && matches!(code, Some(KeyCode::Enter | KeyCode::NumpadEnter)) =>
            {
                if let Some(run) = self.scenes.shop.as_mut() {
                    run.frame = run.scene.length;
                }
                if let Some(a) = self.audio.as_mut() {
                    a.streams.stop_nis(&a.mixer);
                }
                self.shop_scene_frame(0.0);
            }
            Mode::Explore => self.explore_key(bound),
            // UI_ReplayControl's keys (0x4D7BD0, `replay_key`).
            Mode::Race(race) if race.replay.playback.is_some() => self.replay_key(code)?,
            // Pausing in the tuning session (0x89E7E6) opens the last
            // tuning screen made (0x6067A0 → 0x83A650, stored by the
            // screens 0x55A470 / 0x55A630 / 0x55AA50): the dyno menu, which
            // in a test run (0x83A644 = 0) has the test run's options.
            Mode::Race(race) if race.test.is_some() && code == Some(KeyCode::Escape) => {
                // The run is paused under the test run's menu.
                let Mode::Race(race) = std::mem::replace(&mut self.mode, Mode::Explore) else { unreachable!() };
                self.sim.paused_race = Some(race);
                self.mode = self.icon_menu(MenuKind::Dyno);
            }
            // Enter during a race start scene's own camera (before its cut,
            // mark 2) skips to the cut: the chase view and race state 3 at
            // once, the green three steps later, the scene's characters
            // playing on from the cut to the scene's end (**observed**
            // under Wine, raceflow drag1: state 2 → 3 on the key, the green
            // 2.67 s (3 × +0xC 0.889) later, the starter at her cut pose,
            // the player's +0x234 set until 5.5 s past the green). The
            // handler (**verified**) is 0x5FA890, input mode 8 (flow
            // states 2 and 3, 0x5EA660) for the Engage key 0xBF, skip
            // 0xBE and pause 0xBD: in a race it calls NIS_SkipToCut
            // 0x43B460 directly, and the flow's per-frame message 3 then
            // sees the scene past StartCountdown (0x438760): race state 3
            // (0x5FC910(3)) and the flow's 0x603640(4) on the next frame.
            // (Message 6, which also tests 0x7FAAB8, has no poster.)
            // NIS_SkipToCut: the NIS sound object's (0x40000050) "button through" flag
            // 0x82BC49 (0x479200), NIS_SeekTo 0x43AF80 to the
            // StartCountdown event's time (every scene object's time, the
            // scene cars re-placed there by 0x439240, clock +0x2C) and
            // the traffic resumed (0x409A30). The scene is not stopped
            // (0x439070 is not called: +0x234 stays). The scene's line is
            // not simply stopped (**verified**, docs/native-fixes/N-AUD2.tsv): 0x479200 sets the
            // button-through flag and the NIS sound object's update
            // 0x475330 replaces the line with the armed backup line
            // (0x45AD40(7), "Button THrough NIS Playing").
            Mode::Race(_)
                if matches!(code, Some(KeyCode::Enter | KeyCode::NumpadEnter))
                    && self.scenes.shop.as_ref().is_some_and(|r| r.frame < r.scene.marks[1] as f32) =>
            {
                if let Some(run) = self.scenes.shop.as_mut() {
                    let cut = run.scene.marks[1] as f32;
                    let skipped = cut - run.frame;
                    // Native seek 43AFDF places the scene cars at the target
                    // before advancing the clock. A seek is a placement,
                    // not elapsed motion across the skipped interval.
                    if let (Mode::Race(race), Some(d)) = (&mut self.mode, self.sim.drive.as_mut()) {
                        let phases: Vec<_> =
                            std::iter::once(d.state.wheels).chain(race.racers.rivals.iter().map(|r| r.state.wheels)).collect();
                        scene_cars(d, race, &run.scene, run.origin, cut, cut, &phases);
                    }
                    run.frame = cut - 1e-3;
                    if let Some(c) = self.scenes.crowd.as_mut() {
                        c.frame += skipped;
                    }
                }
                if let Some(a) = self.audio.as_mut() {
                    a.streams.nis_button_through();
                }
            }
            Mode::Race(_) => self.race_key(bound),
        }
        Ok(())
    }

    /// `dispatch` in free roam: the pause, the World Map, Engage, the
    /// career status, Read SMS, the camera and the reset.
    fn explore_key(&mut self, acts: Acts) {
        let Acts { pause, camera, reset, world_map, engage, career_stats, read_sms, .. } = acts;
        // While the D-Day script runs only pause (0xBD), skip (0xBE,
        // disabled) and Read SMS (0xC2) are bound (0x605C80(0xA)).
        let held = self.career_world.dday.holds_input();
        if pause {
            self.mode = self.icon_menu(MenuKind::ExplorePause);
        } else if held && !read_sms {
        } else if world_map {
            // 0x602EF5 (code 0xC0): a career with no race (0x500B60:
            // 0x850078 = 1, 0x89E7B0 = 0 — so not in an outrun) and
            // no screen request pending (career +0x7138) posts flow
            // event 0x12: the game pauses (0x60B3B0, mode 6) and
            // 0x606740 opens UI_InGame_WorldMap.
            if let Some(m) = self.in_game_map() {
                self.mode = m;
            }
        } else if engage {
            // 0x602BE0, code 0xBF: the current marker first (state
            // 0x11), else a waiting encounter's challenge (+0x250,
            // 0x500D80 → the outrun).
            if !self.engage()
                && let Some(e) = self.sim.encounters.as_mut()
            {
                e.engage();
            }
        } else if career_stats && !held {
            if let Some(m) = self.status_screen() {
                self.mode = m;
            }
        } else if read_sms
            && self.profile.is_some()
            && !self.race_flow.quick_free_run
            && self.sim.encounters.as_ref().is_none_or(|e| e.outrun.is_none())
            && self.scenes.shop.is_none()
        {
            // 0x602BE0, code 0xC2: in a career's free roam (0x500B60)
            // → 0x501480 (the new flag +0x140C cleared) → state 0xD.
            if let Some(m) = self.sms_open() {
                self.mode = m;
            }
        } else if camera {
            self.cycle_camera();
        } else if reset && let Some(d) = self.sim.drive.as_mut() {
            // 0x602BE0 case 0xA1 takes no race type: free roam resets as a
            // race does (0x6009B0), behind the same gate.
            let racing = self.sim.encounters.as_ref().and_then(|e| e.outrun.as_ref()).is_some();
            let kind = self.sim.roam_camera.as_ref().map_or(0, |c| c.events.kind);
            if reset_key_allowed(d, None, racing, kind) {
                let stage = self.profile.as_ref().map_or(0, |p| u32::from(p.stage));
                let routes = encounter_routes(&self.game_data.caches, &self.game_data.root, stage, None, None).ok();
                let zones = free_roam_reset_zones(&self.game_data.caches, &self.game_data.root);
                reset_in_free_roam(d, routes.as_deref(), zones);
            }
        }
    }

    /// `dispatch` in a race: the pause menu, the camera, the gear shift and
    /// the reset.
    fn race_key(&mut self, acts: Acts) {
        let Acts { pause, camera, reset, up, down, .. } = acts;
        if pause {
            // The race's pause menu (0x4C2BD0) over the frozen race.
            let Mode::Race(race) = std::mem::replace(&mut self.mode, Mode::Explore) else { unreachable!() };
            self.sim.paused_race = Some(race);
            self.mode = self.icon_menu(MenuKind::RacePause);
        } else if camera {
            self.cycle_camera();
        } else if up || down {
            // Shift up / down with a manual gearbox.
            if let Some(d) = self.sim.drive.as_mut() {
                d.state.shift(&d.car, up);
            }
            if let Mode::Race(r) = &mut self.mode
                && let Some(rec) = r.replay.recording.as_mut()
            {
                rec.shift = Some(up);
            }
        } else if reset
            && let (Mode::Race(r), Some(d)) = (&mut self.mode, self.sim.drive.as_mut())
            && reset_key_allowed(d, Some(r), false, r.camera.events.kind)
        {
            reset_to_road(d, r);
            d.stamp_placement();
            if let Some(rec) = r.replay.recording.as_mut() {
                rec.reset = true;
            }
        }
    }

    /// The camera button: the next Camera setting (0x502540: round to
    /// Far, or Super Far in a drift race), kept in the options.
    fn cycle_camera(&mut self) {
        if let Some(d) = self.sim.drive.as_mut() {
            let drift = d.car.car_type.handling.race.drift();
            self.settings.cycle_camera(0, true, drift);
            let _ = self.settings.save();
            d.chase = ChaseCamera::new(&d.car, VIEW_ORDER[self.settings.players[0].camera.min(4) as usize]);
        }
    }
}

/// The Controls rows a press is bound to that free roam and a race read
/// (`dispatch`'s `acts`).
#[derive(Clone, Copy)]
struct Acts {
    pause: bool,
    camera: bool,
    reset: bool,
    up: bool,
    down: bool,
    engage: bool,
    world_map: bool,
    read_sms: bool,
    career_stats: bool,
}

/// The reset key's gate (0x602BE0 case 0xA1 → 0x5F3F10,
/// `racer_reset::reset_key`; **verified** 0x60346E…0x6034BA): no engage
/// (0x5012C0 on 0x85EF50 +0x1C: the showcase's Control Car / Show Off,
/// which the clone runs as a menu, so never here); not while the human
/// driver holds the car on the line (car +0x24 = 1 and [car +0x30] +0x2B4:
/// the race start's hold, 0x410430 / 0x41E37E, `CarState::held`); no
/// blocking event camera (0x8020E8 = 0,0,0,1,1,1,1,1 by kind, player
/// +0x58); at most 60 mph (|v|, car +0x430); not in the first 3 s of
/// racing (race status 4, `Race::clock` from the green) but in an outrun
/// (race type 7: `outrun`); not within the 2 s hold of the last stamped
/// placement ([`Drive::stamp_placement`]).
fn reset_key_allowed(d: &Drive, race: Option<&Race>, outrun: bool, event_kind: u8) -> bool {
    let check = nfsu2::racer_reset::ResetCheck {
        // Player +0x58, free roam's director included.
        event_camera: Some(u32::from(event_kind)).filter(|k| (3..=7).contains(k)),
        speed: d.state.velocity.length(),
        racing_clock: race.map(|r| r.timing.clock).filter(|&c| c >= 0.0),
        outrun,
        since_placed: d.placed_stamp.map(|t| {
            (d.world_clock.tick.wrapping_sub(t) as i32 as f32 * 0.00025, nfsu2::racer_reset::PLACE_HOLD)
        }),
    };
    nfsu2::racer_reset::reset_key(true, false, d.state.held, &check)
}

/// PathsFreeRoam.bin's kind-4 reset zones (0x5D9C00(point, 4, 0) in free
/// roam, read once a run).
fn free_roam_reset_zones<'a>(caches: &'a Caches, root: &std::path::Path) -> &'a [nfsu2::ai::route::ResetZone] {
    caches.reset_zones.get_or_init(|| {
        let path = root.join(format!("TRACKS/ROUTES{EXPLORE_REGION}/PathsFreeRoam.bin"));
        std::fs::read(path).map(|d| nfsu2::ai::route::ResetZone::parse(&d)).unwrap_or_default()
    })
}

/// Racer_ResetCar (0x6009B0, **verified** decompile) in free roam (no race
/// coordinates, 0x88F2BE clear): the car in a kind-4 zone (at car +0x60)
/// goes to the zone's point facing its direction at 8 m/s (0x8021CC);
/// else onto the human's road node when within 150 m (0x43CE10), facing
/// the node's way, at the kept speed max(speed, 0) × max(way · forward, 0),
/// at most 8 m/s; else to the oldest saved place (racer +0xBC + 1 & 3) at
/// rest. Each placement is stamped (flag 1: 0x600DAC / 0x600DF1). The
/// road node is the clone's stand-in for the human controller's +0x0C
/// (the nearest human-flagged control point, as `reset_to_road`;
/// **inferred**); a car on the road drops from 5 m as there.
pub(crate) fn reset_in_free_roam(
    d: &mut Drive,
    routes: Option<&nfsu2::ai::route::AiRoutes>,
    zones: &[nfsu2::ai::route::ResetZone],
) {
    let st = &d.state;
    let at = st.position;
    let forward = st.forward().truncate().normalize_or_zero();
    let mut placed: Option<(Vec3, f32, f32)> = None;
    if let Some(z) = zones.iter().find(|z| z.trigger.contains(at.truncate())) {
        placed = Some((z.trigger.position, z.direction.y.atan2(z.direction.x), RESET_SPEED));
    } else if let Some(routes) = routes {
        let h = st.heading();
        let f = &routes.file;
        if let Some((record, cp, backward)) = nfsu2::ai::route::RacerRoute::find_flagged(
            routes,
            at,
            Vec2::new(h.cos(), h.sin()),
            None,
            true,
            0x0800_0000,
        ) {
            let p = f.records[record].points[cp].position;
            if p.distance(at - Vec3::Z) < 150.0 {
                let dir = f.direction(record, cp, backward);
                let kept = (st.speed().max(0.0) * dir.normalize_or_zero().dot(forward).max(0.0)).min(RESET_SPEED);
                placed = Some((p, dir.y.atan2(dir.x), kept));
            }
        }
    }
    let (p, heading, speed) = placed.unwrap_or_else(|| {
        // The ring fills on the first free-roam update (all four places),
        // so it is never empty here but before that update.
        if d.roam.place_index == 0 {
            (at - Vec3::Z * 5.0, st.heading(), 0.0)
        } else {
            let (p, h) = d.roam.places[(d.roam.place_index.wrapping_add(1) & 3) as usize];
            (p, h, 0.0)
        }
    });
    d.place(p + Vec3::Z * 5.0, heading);
    d.state.set_placement_speed(speed);
    d.stamp_placement();
}
