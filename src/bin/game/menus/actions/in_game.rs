use crate::*;

impl Game {
    /// `act`'s quit / in-game / race group (as `act_main_options`).
    pub(super) fn act_in_game(&mut self, action: Action, kind: MenuKind, now: f32) -> Result<Option<Mode>> {
        Ok(Some(match action {
            Action::MainMenu | Action::QuitToMainMenu => {
                let mut yes = self.icon_menu(MenuKind::Main);
                if let Mode::Menu(Menu::Icons(m, _)) = &mut yes {
                    m.opened = now;
                }
                self.fe.quitting = true;
                self.save_flow.quit_post_race = kind == MenuKind::PostRace;
                if kind.in_game() {
                    // 0x4C2820 / 0x4BED40: UI_InGameDialog, "Quit to Main
                    // Menu?^All unsaved race progress will be lost."
                    // (0xE15B558A; see `in_game_question`).
                    let back = match std::mem::replace(&mut self.mode, Mode::Explore) {
                        m @ Mode::Menu(Menu::Icons(_, k)) if k == kind => m,
                        other => {
                            self.mode = other;
                            self.icon_menu(kind)
                        }
                    };
                    self.in_game_dialog(0xE15B_558A, yes, back)
                } else if kind == MenuKind::Crib {
                    // The crib's (0x4AF80B): its accept (0x0C407210 in the
                    // handler 0x4AF3A0) first plays the Event Handler's
                    // FORWARD (the objects' FADEOUT, then 0xE1FDE1D1 on
                    // which the option's action asks with StartDim
                    // 0x96DDA9E6), as for Return to Explore Mode: under
                    // Wine the question stands over the
                    // garage scene alone, no strip, title or help bar.
                    self.ask_in("GenericDialog_SMALL", 0xCB5E_3178, yes, self.icon_menu(kind))
                } else {
                    dialog_over(self.ask_in("GenericDialog_SMALL", 0xCB5E_3178, yes, self.icon_menu(kind)), true, None)
                }
            }
            // In game.
            Action::ReturnToExplore | Action::ReturnToFreeRun => {
                // UI_PostRace (0x4F39A0 on 0x3411395C): in a career with
                // save prompts on, after a race won (0x5287B0(0)), the
                // outcome is settled (0x54C950) and the save point
                // (0x4A3100 mode 0x441) runs first; its end (0x35E413DD →
                // 0x4A3E30 → 0x54C9E0, settled once) goes on here.
                if kind == MenuKind::PostRace && !std::mem::take(&mut self.save_flow.post_race_saved) && self.post_race_save_due()
                {
                    self.settle_race();
                    self.save_point(false, AfterSave::Act(action, kind))?;
                    return Ok(None);
                }
                // Only UI_PostRace's own exit runs 0x54C9E0 (and so the
                // movie test 0x513A50); the race pause's (0x451E768E →
                // 0x5F41F0(0xB) → 0x513950) and a tournament left early
                // (0x4A3DF0) do not.
                let movie = kind == MenuKind::PostRace && self.race_flow.next_round.is_none();
                if !movie {
                    self.race_flow.event = None;
                }
                return self.return_to_explore().map(|()| None);
            }
            // Jump to Garage (0x4C2170, 0x43DA9FD0 → 0x4A7C70 → 0x513B30):
            // in free roam the car's place is kept, career +0x8744 = 1.
            Action::JumpToGarage => {
                self.drop_challenge();
                if let Some(at) = self.here()
                    && let Some(p) = self.profile.as_mut()
                {
                    p.keep_explore_at(at);
                }
                self.icon_menu(MenuKind::Crib)
            }
            Action::ResumeRace => {
                if let Some(race) = self.sim.paused_race.take() {
                    self.mode = Mode::Race(race);
                }
                return Ok(None);
            }
            Action::Restart => {
                // Restart (0xE1A57D51 → 0x5F41F0(9)) leaves the outcome
                // unsettled: a win neither pays nor counts as won.
                self.drop_race_result();
                let paused = self.sim.paused_race.take();
                let (event, from_map, quick) =
                    paused.as_ref().map(|r| (r.event, r.from_map, r.quick)).or(self.race_flow.last_race).context("no race")?;
                // The race restarted (paused, or finished and kept for its
                // replay; without a recording its end kept the cars): the
                // same field races again, so its rivals' cars are kept for
                // the set-up rather than loaded again.
                if let Some(raced) = paused.or_else(|| self.sim.replay.take()) {
                    self.keep_raced_rivals(raced);
                }
                self.start_race_from(event, quick, from_map, false)?;
                return Ok(None);
            }
            Action::ReturnToWorldMap => {
                // UI_PostRace's (0x33C42E03 → 0x4A3DC0 → 0x54CA50): settled,
                // no save.
                if kind == MenuKind::PostRace {
                    self.settle_race();
                } else {
                    // The pause's (0x4A7CC0): 0x513B30 first.
                    self.drop_challenge();
                }
                self.race_flow.event = None;
                self.sim.paused_race = None;
                self.world_map()
            }
            Action::RaceStats => match self.race_flow.stats.clone() {
                Some(mut stats) => {
                    stats.opened = self.now();
                    stats.index = stats.player;
                    Mode::Menu(Menu::Stats(stats))
                }
                // UI_postrace (and so its Race Stats) only follows a race,
                // whose end files the stats (0x515DB0).
                None => return Ok(None),
            },
            // "Next Race" (0x4BEC90 → 0x5F41F0(10)): the event's next race.
            Action::NextRace => {
                if let Some((event, quick, from_map, round, times)) = self.race_flow.next_round.take() {
                    self.race_flow.map_start = false;
                    // The field goes on to the next race (`Field::looks`):
                    // the race's rivals' cars, kept for its replay, too.
                    if let Some(raced) = self.sim.replay.take() {
                        self.keep_raced_rivals(raced);
                    }
                    self.start_round(event, quick, from_map, round, times)?;
                }
                return Ok(None);
            }
            _ => unreachable!("act_in_game: {action:?}"),
        }))
    }

    /// `act`'s Replay (as `act_main_options`).
    pub(super) fn act_replay(&mut self, action: Action, kind: MenuKind) -> Result<Option<Mode>> {
        Ok(Some(match action {
            // Replay (UI_ReplayControl over the race, 0x4C0AA0): the
            // markers built and a skip to one; the race is then re-run from
            // that snapshot with the recorded inputs. From the post-race
            // menu (0x4BED90; offered only with a recording, 0x4BEE10) the
            // flow object's +0x40 is set (the race's end, 0x60AE55): the
            // first marker (the green light), no THE_PRESENT, and the race
            // camera state is left as it is until the skip's end sets
            // RCSTATE_INSTANT_REPLAY (0x4C10B0 → 0x5F43E0(5)). From the
            // race's pause (UI_pause's Replay, 0x4C2640; +0x40 clear):
            // THE_PRESENT taken (+0xD0, 0x57C090 on the control's first
            // tick) and the last marker (0x4C0F20..0x4C0F46). **Observed**
            // under Wine (2026-10-31, Quick Race 4014): post-race the replay
            // starts at the green (bar 0.69 s, "GO!"), from the pause 5 s
            // back (the last marker, the INSTANT_REPLAY).
            Action::Replay => {
                let pause = kind == MenuKind::RacePause;
                let opened = self.now();
                let race = if pause { self.sim.paused_race.take() } else { self.sim.replay.take() };
                match race {
                    Some(mut race) if race.replay.recording.is_some() && self.sim.drive.is_some() => {
                        let d = self.sim.drive.as_mut().unwrap();
                        let rec = race.replay.recording.as_ref().unwrap();
                        let markers = rec.markers();
                        let present = pause.then(|| Box::new(Snapshot::take(d, &race)));
                        let to = if pause { markers.last() } else { markers.first() };
                        let skip = to.map(|&m| (SkipTo::Lap(m), 0.0));
                        if skip.is_none() {
                            rec.start.clone().restore(d, &mut race);
                        }
                        // The director holds the view showing, the
                        // player's (**observed** under Wine: the chase view
                        // until a track camera or a 4 s draw).
                        let camera = ReplayCamera {
                            view: Some(VIEW_ORDER[self.settings.players[0].camera.min(4) as usize]),
                            ..ReplayCamera::default()
                        };
                        race.replay.playback = Some(Playback {
                            opened,
                            markers,
                            skip,
                            present,
                            pause,
                            paused: skip.is_some(),
                            camera,
                            tracks: track_cameras(&self.game_data.root, &d.region),
                            typed: typed_cameras(&self.game_data.caches, &self.game_data.root),
                            typed_top: d.car.car_type.handling.gearing_top_speed,
                            ..Playback::default()
                        });
                        self.hud_state.hud = Hud::default();
                        Mode::Race(race)
                    }
                    other => {
                        if pause {
                            self.sim.paused_race = other;
                        } else {
                            self.sim.replay = other;
                        }
                        return Ok(None);
                    }
                }
            }
            _ => unreachable!("act_replay: {action:?}"),
        }))
    }
}
