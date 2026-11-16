pub(crate) mod map;
pub(crate) mod views;
#[cfg(all(test, feature = "dev-harness"))]
mod tests;

use crate::*;

impl Game {
    /// The textures the HUD of a drive in `race` on
    /// track `track` (the track record's number, 4000 in free roam) draws
    /// from: its track map (TRACKMAP<n>, loaded now as `map_view` would on
    /// the first HUD frame) and the image resources of its package
    /// (`hud::package_for`), sorted, once each. Read at the load so the HD
    /// UI pack's art for them is made behind the loading screen.
    pub(crate) fn hud_textures(&mut self, race: RaceMode, track: i32) -> Vec<u32> {
        let mut list = Vec::new();
        if let Ok(number) = u16::try_from(track) {
            list.extend(self.game_data.assets.track_map(&self.game_data.root, number));
        }
        let assets = &self.game_data.assets;
        if let Some(p) = assets.package(hud::package_for(race)).or_else(|| assets.package(hud::PACKAGE)) {
            list.extend(
                p.resources
                    .iter()
                    .filter(|r| matches!(r.kind, nfsu2::frontend::feng::ResourceKind::Texture))
                    .filter_map(|r| assets.texture_for(&r.name)),
            );
        }
        // The textures its first frame picks by the car and the race (the
        // custom tachometer's by the rpm and HUDINDEX, the fonts of its
        // text): a throwaway HUD's quads at its start, the drive's car.
        if let Some(d) = &self.sim.drive {
            let h = &d.car.car_type.handling;
            let input = hud::Input {
                options: self.hud_options(race != RaceMode::FreeRoam),
                career: !self.race_flow.quick_free_run && self.profile.is_some(),
                racing: race != RaceMode::FreeRoam,
                race,
                redline: h.engine.redline_rpm,
                max_rpm: h.engine.max_rpm,
                turbo: h.engine.curve_334.values.iter().fold(0.0f32, |m, &v| m.max(v)) > 0.0,
                hud_index: self.hud_index(),
                drag: (race == RaceMode::Drag).then(Default::default),
                ..Default::default()
            };
            list.extend(hud::Hud::default().quads(assets, &input, 0.0).iter().filter_map(|q| q.texture));
        }
        list.sort_unstable();
        list.dedup();
        list
    }

    /// The player's HUD options: the map in a race by the race map mode,
    /// else by the explore map mode (0x5EA8F0).
    fn hud_options(&self, racing: bool) -> hud::HudOptions {
        let p = &self.settings.players[0];
        hud::HudOptions {
            metric: self.settings.metric,
            map: if racing { self.settings.race_map != 0 } else { self.settings.explore_map != 0 },
            gauges: p.gauges,
            position: p.position,
            lap_info: p.lap_info,
            split: p.split != 4,
            score: p.score,
            leaderboard: p.leaderboard,
        }
    }

    /// The fitted custom gauges' HUDINDEX (slot 0xA3, 0x4F56E6).
    fn hud_index(&self) -> u32 {
        self.profile
            .as_ref()
            .and_then(|p| p.installed.get(&0xA3).and_then(|&h| self.game_data.parts.find(&p.car, 0xA3, Some(h), None)))
            .and_then(|part| self.game_data.parts.attribute(part, nfsu2::formats::hash::bin_hash("HUDINDEX")))
            .unwrap_or(0)
    }

    /// The harness's HUD frame at `now` ms: its update ([`Game::hud_update`]),
    /// its sounds posted at once ([`Game::hud_sounds_update`]) and its quads
    /// ([`Game::hud_layout`]). The game runs the update on the tick and the
    /// sounds with the tick's audio (docs/uncapped-fps.md "HUD on ticks").
    #[cfg(any(test, feature = "dev-harness"))]
    pub(crate) fn hud(&mut self, now: f32) -> Vec<Quad> {
        self.hud_update(now);
        self.hud_sounds_update();
        self.hud_layout(now)
    }

    /// The HUD's update at `now` ms (once per tick, `overlay_update`):
    /// the input the HUD reads from the tick's state, the style reports
    /// taken, the cash popup's count, every element's update
    /// (`Hud::step`; its sounds queued for the tick's audio)
    /// and the rear-view mirror's request.
    pub(crate) fn hud_update(&mut self, now: f32) {
        let map = self.map_view();
        let (tricks, race_breaker) = self.style_reports_update();
        let Some(d) = &self.sim.drive else { return };
        let h = &d.car.car_type.handling;
        let train = d.state.drivetrain.as_ref();
        // The player's HUD options: the map in a race by the race map mode,
        // else by the explore map mode (0x5EA8F0).
        // An outrun is a race state (0x89E7B0 = 7) in explore mode.
        let outrun = self
            .sim.encounters
            .as_ref()
            .filter(|_| matches!(self.mode, Mode::Explore))
            .and_then(|e| e.outrun.as_ref().map(|o| (o.clock, views::outrun_hud(e, self.settings.metric))));
        let racing = matches!(self.mode, Mode::Race(_)) || outrun.is_some();
        let options = self.hud_options(racing);
        let mut input = hud::Input {
            options,
            rear_view: self.settings.rearview && !d.chase.looking_back() && self.settings.players[0].camera <= 1,
            career: !self.race_flow.quick_free_run && self.profile.is_some(),
            racers: 1,
            speed: d.state.speed(),
            // The tachometer reads the shown rpm, car +0x440 (0x4C6933).
            rpm: d.state.shown_rpm(),
            redline: h.engine.redline_rpm,
            max_rpm: h.engine.max_rpm,
            turbo: h.engine.curve_334.values.iter().fold(0.0f32, |m, &v| m.max(v)) > 0.0,
            boost: train.map_or(0.0, |t| t.turbo.boost),
            gear: train.map_or(nfsu2::handling::NEUTRAL, |t| t.gear),
            clutch_locked: train.is_none_or(|t| t.shift == nfsu2::handling::Shift::Locked),
            // Hidden while a gear change is under way: 0x4C784E calls
            // 0x5ECD80 (transmission +0x48, `Drivetrain::pending`; Wine,
            // 1 for 0.35–0.51 s per shift) and hides
            // the shift light group +0x30 (0x50CA00) when it is set.
            shift_light: train.is_some_and(|t| t.shift_light(h) && !t.pending),
            cash: self.profile.as_ref().map_or(0, |p| p.cash as i64),
            map,
            name: self.profile.as_ref().map_or_else(|| "Player".to_string(), |p| p.name.clone()),
            hud_index: self.hud_index(),
            // The fitted gauge colours (slots 0xA4 / 0xA5 / 0xA6, 0x4F5100).
            gauge_colors: [0xA4, 0xA5, 0xA6].map(|slot| self.gauge_colour(slot).map(|c| c.1)),
            // The current marker's prompt kind: the HUD queues it when it
            // changes (0x5EAFC0 → 0x4CA020) and drops the last (0x5EAF40 /
            // 0x533BF0 → 0x4DB820). Kept under the in-game screens (the
            // HUD's update does not run there, so nothing is dropped), not
            // in a race (its own HUD).
            engage: self.career_world.marker.filter(|_| !matches!(self.mode, Mode::Race(_))).map(|m| self.prompt_kind(m)),
            // The SMS element (HUD +0x7C) exists in a career, not online.
            sms: self.profile.as_ref().filter(|_| !self.race_flow.quick_free_run).map(|p| hud::SmsHud {
                unread: p.inbox.unread() as u32,
                kind: p.inbox.notify_kind(&self.game_data.career.messages),
            }),
            // The D-Day script's features (0x60031E) and its prompt kind
            // 0x14 (queued in state 1, flashing from state 2).
            script: self.career_world.dday.hud_features().filter(|_| matches!(self.mode, Mode::Explore)),
            phone: self.career_world.dday_prompt.map(|t| self.sim.clock_ms - t),
            // A world challenge in state 1: its clock (0x6004BD, 0x4C9210).
            challenge: self.career_world.challenge.filter(|c| c.state == 1).map(|c| c.time),
            // The style element's +0x79: a tank and the Race Breaker flag.
            style_points: d.state.nitrous.as_ref().is_some_and(|t| t.size > 0) && race_breaker,
            tricks,
            ..Default::default()
        };
        if let Some(reward) = self.hud_state.cash_popup_update(input.career && matches!(self.mode, Mode::Explore)) {
            input.cash_reward = Some(reward);
        }
        // 0x6000A0 in free roam too: its camera director's kinds 1 / 6 / 7
        // give no HUD features (observed under Wine: the HUD gone under a
        // free-roam crash camera).
        if matches!(self.mode, Mode::Explore)
            && let Some(c) = self.sim.roam_camera.as_ref()
        {
            input.event_camera = matches!(c.events.kind, 1 | 6 | 7);
        }
        if let Mode::Race(r) = &self.mode {
            input.racing = true;
            input.finished = r.racers.me.finished;
            input.status = r.racers.me.dropped.unwrap_or(1);
            // 0x4AA1B0 shows the most laps done (+0x09) + 1; the clone
            // keeps it within the race's laps.
            input.lap = (r.racers.me.lap().1.max(0) as u32).min(r.laps.saturating_sub(1));
            input.laps = r.laps;
            input.sprint = (!r.route.loops).then(|| r.progress());
            input.clock = r.timing.clock;
            input.race_age = r.timing.clock - r.timing.origin;
            input.no_countdown = r.scene_start && r.timing.clock < r.scene_end;
            input.countdown_step = r.timing.countdown_step;
            // A replay: the countdown element alone (0x6000A0 in
            // RCSTATE_INSTANT_REPLAY), no mirror.
            input.replay = r.replay.playback.is_some();
            // 0x6000A0: event kinds 1 / 6 / 7 give no HUD features.
            input.event_camera = r.replay.playback.is_none() && matches!(r.camera.events.kind, 1 | 6 | 7);
            // 0x5EEA50 gives "GO!" from the green tick (+4) alone, which
            // every race's green light sets (0x5FC910(4)); the start
            // scene only leaves +8 (the 3-2-1) at 0. Observed under Wine:
            // "GO!" at a replay's green light in circuit, drag and drift.
            input.no_countdown &= !input.replay;
            input.rear_view &= !input.replay;
            input.lap_start = r.racers.me.lap_start;
            input.last_lap = r.racers.me.last_lap;
            input.best_lap = r.racers.me.best_lap;
            input.laps_done = r.racers.me.lap().1.max(0) as u32;
            input.wrong_way = r.wrong_way;
            input.race = r.test.map_or_else(|| self.game_data.career.events[r.event].race_mode(), |t| t.1);
            input.drift = views::drift_hud(r);
            // The position element and leader board (0x4EBB10, 0x4C6F30).
            let standings = r.standings();
            let laps = if r.route.loops { r.laps.max(1) } else { 1 };
            input.racers = standings.len() as u32;
            // NumberRacers less the knocked out (race status +0x28) in
            // race types 3 and 7 (0x4EBB10; **observed** under Wine,
            // finish151: "1st /3" after a Lap KO elimination).
            if r.scoring.knockout {
                let out = std::iter::once(&r.racers.me)
                    .chain(r.racers.rivals.iter().map(|x| &x.progress))
                    .filter(|p| p.dropped == Some(3))
                    .count() as u32;
                input.knocked_out = out;
            }
            input.field = r.racers.rivals.len() as u32 + 1;
            input.place = r.order.iter().position(|&i| i == 0).map_or(1, |p| p as u32 + 1);
            // The finish order (0x5EEF80: the racers finished before, + 1).
            input.finish_order = r.racers.me.finish_order(r.racers.rivals.iter().map(|r| &r.progress));
            input.race_time = r.timing.clock.max(0.0);
            input.hurry = r.hurry_left();
            input.leaders = r
                .order
                .iter()
                .map(|&i| {
                    let s = &standings[i];
                    hud::LeaderRow {
                        name: if i == 0 { input.name.clone() } else { r.racers.rivals[i - 1].name.clone() },
                        player: i == 0,
                        gap: nfsu2::standings::gap(s, &standings[0], &r.reference, laps, r.timing.clock),
                        done: s.finished.map(|_| views::status_label(if i == 0 { &r.racers.me } else { &r.racers.rivals[i - 1].progress })),
                        points: None,
                    }
                })
                .collect();
            // A drift race (0x4EBB10's drift branch, 0x4DA190 → 0x4C6EC0 /
            // 0x4AAC50 / 0x4AADC0): the place is the player's node in the
            // points list (0x56DDA0) of its count (0x43BE80) racers, and the
            // leader board shows its first four nodes: "%d" each one's
            // points for 10 s, then the names for 2 s, SetPlayerColor on
            // the nodes with the flag (+0x14).
            if !r.scoring.board.is_empty() {
                input.racers = r.scoring.board.len() as u32;
                input.place = nfsu2::drift::board_place(&r.scoring.board, 0).unwrap_or(1) as u32;
                input.leaders = r
                    .scoring.board
                    .iter()
                    .take(4)
                    .map(|e| hud::LeaderRow {
                        name: if e.racer == Some(0) { input.name.clone() } else { e.name.clone() },
                        player: e.flag,
                        gap: 0,
                        done: None,
                        points: Some(e.score as i32),
                    })
                    .collect();
            }
            if input.race == nfsu2::race::RaceMode::Drag {
                input.drag = Some(hud::DragHud {
                    shift_quality: train.map_or(0.0, |t| t.shift_bonus),
                    // The tachometer's last gear (HUD +0x80) and the shown rpm.
                    shift_light: train.and_then(|t| t.drag_shift_light(h, self.hud_state.hud.drag_last_gear())),
                    needle_blend: train.map_or(0.0, |t| t.drag_needle_blend(h, self.hud_state.hud.drag_last_gear())),
                    heat: train.map_or(0.0, |t| t.heat_fraction()),
                    needle: h.drag_needle,
                });
            }
        }
        input.nitrous = d.state.nitrous.filter(|t| t.size > 0).map(|t| t.fraction());
        // Career free roam's encounters: the encounter icon while the
        // manager tries to engage, an outrun's race (0x4EBB10: place, the
        // racers less those done, so "/2"; the race time from 0x607190)
        // and its race-over message (0x5FFF60, 3 s).
        if let Some(e) = self.sim.encounters.as_ref().filter(|_| matches!(self.mode, Mode::Explore)) {
            input.encounter = e.icon && e.state == 3;
            if let Some((clock, Some(o))) = outrun {
                input.racing = true;
                input.outrun = Some(o);
                input.racers = 2;
                input.place = if o.leading { 1 } else { 2 };
                input.race_time = clock;
                input.clock = clock;
            } else if let Some((won, bank, time)) = e.race_over() {
                input.finished = Some(time);
                input.outrun_result = Some((won, bank));
            }
        }
        self.hud_state.hud.step(&self.game_data.assets, &input, now);
        self.rear_view_update();
    }

    /// The HUD's quads at `now` ms from its state as the last tick's
    /// update left it (`hud_update`): a pure read, drawn between ticks.
    pub(crate) fn hud_layout(&self, now: f32) -> Vec<Quad> {
        if self.sim.drive.is_none() {
            return Vec::new();
        }
        let mut quads = self.hud_state.hud.layout(&self.game_data.assets, now);
        let Some(font) = self.game_data.assets.fonts.values().find(|f| f.name.contains("tt21i")) else { return quads };
        let white = [1.0; 4];
        if let Some((text, until)) = &self.hud_state.toast
            && now < *until
        {
            text_quads(&self.game_data.assets, font, text, 1, Vec2::splat(0.6), Vec2::new(320.0, 380.0), white, 0.0, &mut quads);
        }
        quads
    }

    /// The tricks the player's style manager reported (0x56FD20
    /// → 0x4DA6F0), taken once, and the Race Breaker flag (0x89E7E7: 1 in
    /// free roam, observed). Called from `hud_update`.
    pub(crate) fn style_reports_update(&mut self) -> (Vec<nfsu2::drift::TrickReport>, bool) {
        match &mut self.mode {
            Mode::Race(r) => {
                (r.scoring.style.as_mut().map(|s| std::mem::take(&mut s.reports)).unwrap_or_default(), r.scoring.race_breaker)
            }
            _ => (self.sim.roam_style.as_mut().map(|s| std::mem::take(&mut s.0.reports)).unwrap_or_default(), true),
        }
    }

    /// The engage prompt's sounds (0x51DE40(FOCUS, 0xD / 0xE /
    /// 0xF): FE ids 14, 15, 16) as its slots flash and step, queued by the
    /// tick's HUD update (0x600910, step 16 of the loop 0x5811D0) and
    /// posted at the end of the tick's sounds (`audio_sounds`; the audio
    /// manager 0x483B40 is step 18), so the capture and `aud` have them at
    /// the tick.
    pub(crate) fn hud_sounds_update(&mut self) {
        for (event, control) in self.hud_state.hud.take_sounds() {
            self.fe_sound(event, control);
        }
    }

    /// HUD_FEATURE_RVM makes the mirror's mover (0x4CA090 →
    /// 0x5F1E20) and 0x5C21F0 draws view 3 while view 1's mover is not a
    /// chase kind (vtable +0x24); the player's own view (not a replay's or
    /// the showcase's camera). Called from `hud_update` after the HUD's step.
    pub(crate) fn rear_view_update(&mut self) {
        self.hud_state.rear_view_on = self.hud_state.hud.features().0 & 0x1000 != 0
            && native_mover_type(&self.mode, self.sim.drive.as_ref()) == 1
            && self.sim.drive.as_ref().is_some_and(|d| !d.chase.is_chase_kind());
    }

    /// The mirror off until the HUD's update asks for it again
    /// (`rear_view_update`). Called first in `overlay_update`.
    pub(crate) fn rear_view_reset_update(&mut self) {
        self.hud_state.rear_view_on = false;
    }

    /// The overlay's update on the tick at `now` ms (`Game::step`,
    /// after the update and before the tick's sounds: the original's HUD
    /// update 0x600910 is step 16 of its loop 0x5811D0, after the world
    /// update (13) and the package clock (15), before the audio manager
    /// (18) and the render (24); measured under Wine). What it steps is what
    /// `overlay_layout` then draws: the HUD (a replay's under its bar),
    /// nothing under a shop or start scene's letterbox or a menu; under the
    /// pause the HUD's pause update (`Hud::pause_step`).
    pub(crate) fn overlay_update(&mut self, now: f32) {
        // Set again by the HUD's update when it asks for the mirror.
        self.rear_view_reset_update();
        match &self.mode {
            // The pause (flow state 6): the HUD update under the pause
            // features (0x6000A0 → lo 0x40000, hi 0); the outrun meter's
            // count kept only in an outrun (race type 7).
            Mode::Menu(Menu::Icons(_, MenuKind::ExplorePause | MenuKind::RacePause | MenuKind::PauseOptions(_)))
                if self.sim.drive.is_some() =>
            {
                let outrun = self.sim.encounters.as_ref().is_some_and(|e| e.outrun.is_some());
                self.hud_state.hud.pause_step(now, outrun);
            }
            Mode::Menu(_) => {}
            Mode::Race(r) if r.replay.playback.is_some() => self.hud_update(now),
            Mode::Explore if self.scenes.shop.is_some() => {}
            Mode::Race(_) if self.scenes.shop.as_ref().is_some_and(|r| r.frame <= r.scene.marks[1] as f32) => {}
            _ => self.hud_update(now),
        }
    }

    /// The harness's overlay frame at `now` ms: `overlay_update`, the HUD's
    /// sounds posted at once and `overlay_layout` (what the window's frame
    /// did in one call before the HUD moved onto the tick).
    #[cfg(any(test, feature = "dev-harness"))]
    pub(crate) fn overlay_quads(&mut self, now: f32) -> Vec<Quad> {
        self.overlay_update(now);
        self.hud_sounds_update();
        self.overlay_layout(now)
    }

    /// What is drawn over the running game: the HUD, or a replay's
    /// control bar; nothing under a menu. A pure read of the state the
    /// last tick's `overlay_update` left: drawn between ticks, the
    /// last tick's HUD (stepped).
    pub(crate) fn overlay_layout(&self, now: f32) -> Vec<Quad> {
        match &self.mode {
            Mode::Menu(_) => Vec::new(),
            Mode::Race(r) if r.replay.playback.is_some() => {
                let quads = self.hud_layout(now);
                self.replay_quads(quads, now)
            }
            // A shop drive-in / drive-out scene and the career's start
            // scene show no HUD (both seen in the original under Wine,
            // scenes75 and 2026-10-29; the gate in the HUD's code is not
            // traced).
            Mode::Explore if self.scenes.shop.is_some() => self.letterbox_quads(),
            // A race's start scene: race status 2, the HUD's letterbox
            // only (0x6000A0 → 0x600525), through the scene frame at mark
            // 2 (header +0x30) inclusive — status 3 starts the frame after
            // the scene clock passes it (Wine, probe
            // at 0x437741: clock 6.02 > 6.0).
            Mode::Race(_) if self.scenes.shop.as_ref().is_some_and(|r| r.frame <= r.scene.marks[1] as f32) => {
                self.letterbox_quads()
            }
            _ => {
                let mut quads = self.hud_layout(now);
                // The Enhancements page's HUD Scale and the per-element
                // scales and Reduced Flashing (clone deviations; native
                // values leave the quads untouched).
                nfsu2::extras::apply_hud(&mut quads);
                quads
            }
        }
    }

    /// The scene camera's letterbox (`track_camera::letterbox`): a black
    /// bar of that share of the screen height at the top and the bottom.
    /// Native 0x5CBDD0 (from the frame's post-FX pass 0x5CBF50, before the
    /// front end 0x5378D0): with view 1's camera +0xC8 > 0, h =
    /// _ftol(+0xC8 × 480) and two FE quads (0x5C5030, WHITE16X16_NOALPHA
    /// at colour 0) over (0, 0)–(640, h) and (0, 480 − h)–(640, 480) on
    /// the 640 × 480 canvas: the 3D view is drawn full and covered, not
    /// narrowed (the projection 0x5C7ED0 and viewport 0x5BA3D0 ignore it).
    pub(crate) fn letterbox_quads(&self) -> Vec<Quad> {
        let Some(run) = self.scenes.shop.as_ref() else { return Vec::new() };
        let frame = match &self.mode {
            // The race start scene's camera runs over the frames up to
            // its second mark (`Game::update`'s race scene).
            Mode::Race(_) => {
                let cut = run.scene.marks[1] as f32;
                let Some(t) = run.camera.as_ref() else { return Vec::new() };
                let fraction = run.frame / cut.max(1.0);
                let Some(i) = nfsu2::track_camera::entry_index(t, fraction).filter(|&i| t.entries[i].mover > 1) else {
                    return Vec::new();
                };
                return bars(nfsu2::track_camera::letterbox(t, i, fraction));
            }
            _ => run.frame.min(run.scene.length),
        };
        let Some(t) = run.camera.as_ref() else { return Vec::new() };
        return bars(run.scene.letterbox(t, frame));
        fn bars(share: f32) -> Vec<Quad> {
            if share <= 0.0 {
                return Vec::new();
            }
            let (w, h) = (nfsu2::frontend::CANVAS.x, nfsu2::frontend::CANVAS.y);
            let bar = |y0: f32, y1: f32| Quad {
                texture: None,
                rect: [Vec2::new(0.0, y0), Vec2::new(w, y1)],
                uv: [Vec2::ZERO, Vec2::ONE],
                color: [0.0, 0.0, 0.0, 1.0],
                z: 0.0,
                angle: 0.0,
                place: Default::default(),
            };
            let px = (h * share).trunc();
            vec![bar(0.0, px), bar(h - px, h)]
        }
    }

    /// A replay's screen: UI_ReplayControl over the HUD's countdown
    /// alone — the replay screen sets RCSTATE_INSTANT_REPLAY (5; 0x5F4250
    /// at 0x4C0EFA), and 0x6000A0 in that state gives the low features
    /// none (0x60018B) and the high the countdown element 0x20000 (the
    /// tail 0x60052C), so the restored green light shows "GO!" for a step
    /// (**verified**; **observed** under Wine from the post-race menu).
    /// `quads`: the HUD's (`hud_layout`, by the caller).
    pub(crate) fn replay_quads(&self, mut quads: Vec<Quad>, now: f32) -> Vec<Quad> {
        let Mode::Race(race) = &self.mode else { return Vec::new() };
        let (Some(play), Some(rec)) = (race.replay.playback.as_ref(), race.replay.recording.as_ref()) else { return Vec::new() };
        if play.hidden {
            return quads;
        }
        let bar = screens::ReplayBar {
            time: rec.time_at(play.at),
            length: rec.elapsed,
            lap: (race.racers.me.lap().1 + 1).max(1) as u32,
            laps: race.laps,
            paused: play.paused,
            markers: play.markers.iter().map(|&m| rec.mark(m).time).collect(),
            drag: race.drag,
        };
        quads.extend(screens::replay_control_quads(&self.game_data.assets, &bar, play.opened, now));
        quads
    }
}

impl HudState {
    /// 0x4A9FF0 (career free roam, HUD +0x78; `career_roam`):
    /// counts 0x8364AC down each update and shows the popup at 0 (its kind
    /// and cash for the HUD's input). Called from `Game::hud_update`, once
    /// a tick; not under the pause (0x4A9FF0 returns before the
    /// count without its feature).
    pub(crate) fn cash_popup_update(&mut self, career_roam: bool) -> Option<(u8, i32)> {
        if career_roam && let Some((kind, cash, wait)) = self.cash_popup.as_mut() {
            *wait = wait.saturating_sub(1);
            if *wait == 0 {
                let reward = (*kind, *cash);
                self.cash_popup = None;
                return Some(reward);
            }
        }
        None
    }
}

/// The HUD's running state (were `Game` fields). Every field starts
/// empty (`Default`).
#[derive(Default)]
pub(crate) struct HudState {
    /// A message shown on the HUD until this time (ms).
    pub(crate) toast: Option<(String, f32)>,
    /// HUD_SingleRace's running state.
    pub(crate) hud: Hud,
    /// A photo shoot's HUD cash popup (0x541220 → 0x4AC170: 0x8364A8 the
    /// milestone's kind, 0x8364A0 + 0x8364A4 the cash, 0x8364AC = 4 HUD
    /// updates to wait, 0x83649C set).
    pub(crate) cash_popup: Option<(u8, i32, u8)>,
    /// The last HUD update asked for the rear-view mirror (HUD_FEATURE_RVM)
    /// and view 1's mover is not a chase kind (0x5C21F0: screen layout 2,
    /// view 3 drawn).
    pub(crate) rear_view_on: bool,
}
