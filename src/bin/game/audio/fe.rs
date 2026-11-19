use crate::*;

impl Game {
    /// PlayFESound (0x458520) with a sound id: under 23 FE_COMMON's
    /// CommonSample, else FE_MB's FrontEndSample id − 23.
    pub(crate) fn fe_sample(&mut self, id: u8) {
        self.fe_id(id);
    }

    /// SFXObj_FEHUD's slots as the FE events' level parameter: the mix
    /// map's levels already hold the effects and master volumes
    /// (SFXCTL_MasterVol → control 3), and with the map loaded the
    /// mixer's effects bus is 1 (`stream_sounds`).
    pub(crate) fn fe_event_levels(&self) -> [i32; 8] {
        let Some(a) = self.audio.as_ref() else { return [0; 8] };
        a.fe_levels
    }

    /// The minimap's GPS text (0x4DA640 → 0x4C8410, run in a career's free
    /// roam with the GPS engaged): PlayFESound 0x12 (FE_COMMON's
    /// CommonSample 18) when the GPS state (0x501100) turns 3, the route
    /// found (element +0x35 the last update's "state 3").
    pub(crate) fn gps_sound(&mut self) {
        let shown = matches!(self.mode, Mode::Explore) && self.profile.is_some() && !self.race_flow.quick_free_run;
        let found = shown && self.career_world.gps.as_ref().is_some_and(|g| g.state() == 3);
        let Some(a) = self.audio.as_mut() else { return };
        // The element runs only while shown; its flag keeps otherwise.
        if !shown {
            return;
        }
        let rising = found && !a.gps_found;
        a.gps_found = found;
        if rising {
            self.fe_sample(0x12);
        }
    }

    /// The engage dialogs' "ready" (0x35F8620B, which the package's own
    /// script sends; `DialogFocus`): UI_EngageRaceDialog (0x499C00),
    /// UI_EngageMessageDialog (0x497B00) and UI_SMS_Mailbox (0x4CCC10) play
    /// id 15 (0x50D180), UI_EngageShopDialog (0x499690, also the
    /// challenge's retry) table [BACK][0] (id 5); UI_EngageEventDialog
    /// (0x498F00) only moves the focus.
    pub(crate) fn dialog_ready_sound(&mut self) {
        use nfsu2::audio::fe_rules as r;
        let shown = match &self.mode {
            Mode::Menu(Menu::Engage(e)) => Some(("UI_EngageRaceDialog", e.opened, r::READY_RACE)),
            Mode::Menu(Menu::ShopEngage(e)) => Some(("UI_EngageShopDialog", e.opened, r::READY_SHOP)),
            Mode::Menu(Menu::Challenge(e)) if matches!(e.kind, screens::ChallengeDialogKind::Retry) => {
                Some((e.package(), e.opened, r::READY_SHOP))
            }
            Mode::Menu(Menu::SmsViewer(v)) => Some(("UI_EngageMessageDialog", v.opened, r::READY_RACE)),
            Mode::Menu(Menu::SmsMailbox(m)) => Some(("UI_SMS_Mailbox", m.opened, r::READY_RACE)),
            _ => None,
        };
        let now = self.now();
        let Some((package, opened, id)) = shown else {
            if let Some(a) = self.audio.as_mut() {
                a.dialog_ready = None;
            }
            return;
        };
        let Some(a) = self.audio.as_ref() else { return };
        let state = match a.dialog_ready {
            Some(s) if s.0 == opened => s,
            _ => (opened, self.game_data.assets.package(package).and_then(|p| nfsu2::frontend::host_time(p, 0x35F8_620B)), false),
        };
        let fire = !state.2 && state.1.is_some_and(|t| now - opened >= t);
        if let Some(a) = self.audio.as_mut() {
            a.dialog_ready = Some((state.0, state.1, state.2 || fire));
        }
        if fire {
            self.fe_id(id);
        }
    }

    /// The FE car's turn sound (0x4D82D0 at 0x4D8BE6): while the car's
    /// angle (+0x274) differs from the last update's (+0x170) PlayFESound
    /// 0x49 starts once (FE_MB's FrontEndSample_loop, index 50: a sample
    /// looped over its whole length); the first update it stands still
    /// StopFESound 0x49 (0x458560) stops it.
    pub(crate) fn car_turn_sound(&mut self, angle: f32, rate: f32) {
        let levels = self.fe_event_levels();
        let Some(a) = self.audio.as_mut() else { return };
        let (last, playing, stopped) = &mut a.car_turn;
        if angle != *last {
            *last = angle;
            // 0x4D8C31: 0x7EC858 = |+0x248| while the angle changes.
            a.car_turn_rate = rate.abs();
            if !*playing {
                a.fe.play(&mut a.mixer, 0x49, &levels);
                *playing = true;
                *stopped = false;
            }
        } else if !*stopped {
            a.fe.turn_stop();
            *stopped = true;
            *playing = false;
        }
    }

    /// A shop or ending scene's NIS line (NisAudio_Request 0x484510 kind 2
    /// from the NIS load 0x435BE0, gated there on 0x827C38 and 0x7FAAA8,
    /// taken as set): NIS_Select_End by [`nfsu2::audio::speech::nis::end_tuple`],
    /// the first fitting record of event 0x4011.
    pub(crate) fn scene_line(&mut self, scene: &str, track: u32) {
        let Some(tuple) = nfsu2::audio::speech::nis::end_tuple(scene, track) else { return };
        let Some(a) = self.audio.as_mut() else { return };
        let played = a.streams.play_nis(&a.mixer, nfsu2::audio::speech::nis::EVENT_END, &tuple, 6);
        if debug_env().audio_debug {
            eprintln!("audio: scene line {scene} tuple {tuple:x?} {}", if played { "plays" } else { "fits no record" });
        }
    }

    /// PlayFESound (0x458520) with a sound id.
    pub(crate) fn fe_id(&mut self, id: u8) {
        if self.platform.mouse_state.mute {
            return;
        }
        #[cfg(test)]
        self.dev.fe_played.push(id);
        if debug_env().audio_debug {
            eprintln!("audio: FE sound {id}");
        }
        let levels = self.fe_event_levels();
        let Some(a) = self.audio.as_mut() else { return };
        a.fe.play(&mut a.mixer, id, &levels);
    }

    /// A fitted part's sound (FE_PlayInstallSound 0x521290(part), after
    /// 0x55C240 fits it): slot 0x10 (exhaust tips) table [INSTALL][EXHAUST]
    /// (0x838150, id 49); slot 0x36 (rear window decal) id 84, then
    /// [INSTALL][GENERIC] (0x83813F, id 50); any other slot id 50.
    pub(crate) fn install_sound(&mut self, slot: u8) {
        use fe::{control, event};
        match slot {
            0x10 => self.fe_sound(event::INSTALL, control::EXHAUST),
            0x36 => {
                self.fe_id(84);
                self.fe_sound(event::INSTALL, control::GENERIC);
            }
            _ => self.fe_sound(event::INSTALL, control::GENERIC),
        }
    }

    /// The sound the current screen's own handler plays for `key`, for the
    /// screens whose handler is traced (`audio::fe_rules`, sound round 7):
    /// `Some(None)` when it plays nothing, `None` for the screens still
    /// on the control-kind rule.
    pub(crate) fn native_menu_sound(&self, key: Key) -> Option<Option<u8>> {
        use nfsu2::audio::fe_rules as r;
        let Mode::Menu(menu) = &self.mode else { return None };
        let now = self.now();
        Some(match menu {
            Menu::Options(o) => o.sound(key, &self.settings, FORCE_FEEDBACK),
            Menu::QrOptions(o) => self.qr_options_sound(key, o),
            // UI_PerformanceBrandSelect's rows: option vtable 0x79B6B0
            // (+0x38 0x50D500, +0x3C 0x408CD0), always enabled.
            Menu::BrandSelect(b, _) => r::list_screen(key, b.rows.get(b.selected).map(|_| (r::RowClass::Toggle, true))),
            Menu::TuneSliders(_) => r::tuning(key),
            Menu::Engage(e) => {
                r::race_dialog(key, e.focus.accept_focused(&self.game_data.assets, "UI_EngageRaceDialog", e.opened, now))
            }
            Menu::ShopEngage(e) => {
                r::shop_dialog(key, e.focus.accept_focused(&self.game_data.assets, "UI_EngageShopDialog", e.opened, now), false)
            }
            Menu::Challenge(e) => {
                let focus = e.focus.accept_focused(&self.game_data.assets, e.package(), e.opened, now);
                match e.kind {
                    screens::ChallengeDialogKind::Offer { .. } => r::event_dialog(key, focus),
                    // A photo shoot's spot: the same package and handler
                    // (0x498F00's milestone branch), so the offer's rule
                    // (**inferred** for its Back, which does nothing there).
                    screens::ChallengeDialogKind::Milestone { .. } => r::event_dialog(key, focus),
                    screens::ChallengeDialogKind::Retry => r::shop_dialog(key, focus, true),
                }
            }
            Menu::SmsViewer(_) => r::message_dialog(key),
            Menu::SmsMailbox(m) => r::mailbox(
                key,
                r::MailboxState {
                    first_box: m.mailbox == 0,
                    last_box: m.mailbox + 1 >= nfsu2::frontend::sms::MAILBOXES.len(),
                    rows: m.rows.len(),
                    row: m.selected,
                },
            ),
            Menu::WorldMap(m) => r::world_map(
                key,
                r::WorldMapState {
                    event: if m.region { 0 } else { m.selected + 1 },
                    events: m.events.len() + 1,
                    first_stage: m.stage <= 1,
                    last_stage: m.stage >= m.stages,
                },
            ),
            Menu::QuickRace(_) => r::track_select(key),
            // Sound round 8.
            Menu::CarLot(l) => self.car_lot_sound(key, l, now),
            Menu::InGameMap(m) => {
                let changed = matches!(key, Key::Left | Key::Right) && {
                    let mut next = m.clone();
                    next.key(key, self.game_data.assets.package("UI_InGame_WorldMap"));
                    next.selected != m.selected
                };
                r::in_game_map(key, r::InGameMapState { gps: m.gps, changed, focused: m.focus.is_some() })
            }
            Menu::Status(st) => r::status(key, st.screen.page, st.screen.pages),
            Menu::Stats(st) => r::post_race_stats(key, st.index, st.racers.len()),
            Menu::Dialog(d, _) => r::dialog(key, d.buttons.len(), d.back),
            Menu::Paint(p) => Self::paint_sound(key, p),
            Menu::Parts(b) if Self::strip_shop(b.package).is_some() => {
                let (shop, s) = Self::strip_state(b)?;
                r::strip_shop(shop, key, s)
            }
            Menu::Parts(b) => self.parts_browser_sound(key, b),
            // UI_ChooseCustomHUD (handler 0x5644E0, filter 0x570A80) has
            // no call sites: silent.
            Menu::ChooseHud(_) => None,
            Menu::Controls(c) => r::controls(key, c.selected),
            Menu::Jukebox(_) => r::jukebox(key),
            Menu::Showcase(v) => match v.mode {
                screens::ShowcaseMode::Menu => r::showcase_menu(key, v.option),
                screens::ShowcaseMode::Picture => r::showcase_picture(key),
                // Leaving a drive: EngageState_Stop's BACK (`showcase_key`);
                // Show Off's part keys: 0x61F830's in-game stitch
                // (`part_stitch`).
                _ => None,
            },
            Menu::Cover(_) => r::cover(key),
            // UI_BuyPerformanceParts under Choose Individual Parts
            // (0x569EF0, +0x1E6 clear): the list base 0x556030 and its own
            // Up / Down (0x562540, then [UP][0] 0x56A14F / 0x51DE70(_, 7)
            // 0x56A20C) — UP / DOWN on every press.
            Menu::Icons(m, MenuKind::PerfPackage) if m.selected == 0 && matches!(key, Key::Up | Key::Down) => {
                fe::sound(if key == Key::Up { fe::event::UP } else { fe::event::DOWN }, 0)
            }
            Menu::Reward(_) => r::quiet_screen(r::QuietScreen::Reward, key),
            Menu::Results(..) => r::quiet_screen(r::QuietScreen::Results, key),
            Menu::SponsorPopup(_) => r::quiet_screen(r::QuietScreen::SponsorPopup, key),
            Menu::DynoResults(_) => r::quiet_screen(r::QuietScreen::DynoResults, key),
            Menu::McList(_) => r::quiet_screen(r::QuietScreen::McList, key),
            Menu::Credits(_) => r::quiet_screen(r::QuietScreen::Credits, key),
            Menu::TuneGraph(_) => r::quiet_screen(r::QuietScreen::TuneGraph, key),
            Menu::Splash(sp) => r::splash(key, sp.ready.is_some()),
            // Sound round 9: MC_Main (filter 0x4A33D0): its questions'
            // Yes / No (0x4A3890, mode 1); the name editor keeps them
            // prov: inferred game.native_menu_sound
            // (state 3 only hides the button slots, 0x4A3460: **inferred**
            // that the editor lets the pad messages through); the
            // memory-card messages by their button count (0x4C6050 →
            // 0x4A35C0).
            Menu::McMain(_) => r::mc_main(key, r::McButtons::Two),
            Menu::McMessage(m, _) => {
                r::mc_main(key, if m.buttons.len() >= 2 { r::McButtons::Two } else { r::McButtons::One })
            }
            Menu::DynoChart(c) => r::dyno_chart(key, c.pull.is_some() && !c.recorded && c.progress() < 1.0),
            // Still open: the online stubs.
            _ => return None,
        })
    }

    /// An arm of `native_menu_sound`.
    fn qr_options_sound(&self, key: Key, o: &nfsu2::frontend::quick_race::QrOptions) -> Option<u8> {
        use nfsu2::audio::fe_rules as r;
        // UI_QRModeOptions' rows are all toggles (0x4B3110…0x4B49C3:
        // +0x3C 0x408CD0, +0x38 0x50D500).
        let ty = nfsu2::frontend::quick_race::race_type(o.mode);
        let q = &self.settings.qr[ty];
        let profile = self.profile.as_ref();
        let data = &self.game_data.career;
        let open = nfsu2::frontend::quick_race::unlocked(data, q.track, &|e| {
            profile.is_some_and(|p| p.won(data, e.unlock))
        });
        let rows = nfsu2::frontend::quick_race::rows(ty, data.tracks.get(&q.track), open, q);
        r::list_screen(key, rows.get(o.selected).map(|row| (r::RowClass::Toggle, row.enabled)))
    }

    /// An arm of `native_menu_sound`.
    fn car_lot_sound(&self, key: Key, l: &CarLot, now: f32) -> Option<u8> {
        use nfsu2::audio::fe_rules as r;
        let s = r::LotState { first: l.selected == 0, last: l.selected + 1 >= l.cars.len() };
        match l.state {
            LotState::CribSelect => r::crib_select(key, s),
            LotState::QuickRace(_) | LotState::Customize => {
                // +0x1A7: the package's 0x35F8620B.
                let ready = self
                    .game_data.assets
                    .package("UI_QRCarSelect")
                    .and_then(|p| nfsu2::frontend::host_time(p, 0x35F8_620B))
                    .is_none_or(|t| now - l.opened >= t);
                r::qr_car_select(key, ready)
            }
            _ => r::car_lot(key, s),
        }
    }

    /// An arm of `native_menu_sound`.
    fn paint_sound(key: Key, p: &screens::PaintShop) -> Option<u8> {
        use nfsu2::audio::fe_rules as r;
        use nfsu2::frontend::screens::{GLOSS, METAL, PEARL};
        let n = p.slots().len();
        r::paint(
            key,
            r::PaintState {
                in_slot: p.selected.is_some(),
                first: p.slot == 0,
                last: p.slot + 1 >= n,
                swatches: !p.swatches.is_empty(),
                finish_cycles: matches!(p.finish, GLOSS | METAL | PEARL),
            },
        )
    }

    /// An arm of `native_menu_sound`.
    fn parts_browser_sound(&self, key: Key, b: &PartsBrowser) -> Option<u8> {
        use nfsu2::audio::fe_rules as r;
        let c = b.categories.get(b.category);
        let s = match b.part {
            Some(i) => {
                let n = c.map_or(0, |c| c.parts.len());
                r::BrowserState {
                    in_category: true,
                    first: i == 0,
                    last: i + 1 >= n,
                    locked: false,
                    empty: n == 0,
                    customize: false,
                }
            }
            None => r::BrowserState {
                in_category: false,
                first: b.category == 0,
                last: b.category + 1 >= b.categories.len(),
                locked: c.is_some_and(|c| c.locked),
                empty: c.is_none_or(|c| {
                    c.parts.is_empty()
                        && !matches!(
                            c.action,
                            screens::CategoryAction::Open | screens::CategoryAction::Toggle(_)
                        )
                }),
                customize: self.save_flow.customize.is_some(),
            },
        };
        r::parts_browser(key, s)
    }

    /// The shop browser on an IconScrollerMenu strip with its own handler
    /// (sound round 9), by the clone's package.
    fn strip_shop(package: &str) -> Option<nfsu2::audio::fe_rules::StripShop> {
        use nfsu2::audio::fe_rules::StripShop;
        Some(match package {
            "UI_Rims_Browser" => StripShop::Rims,
            "UI_ICEMAIN" => StripShop::Ice,
            "UI_CustomNeonMain" => StripShop::Neon,
            "UI_DecalMain" => StripShop::Decal,
            _ => return None,
        })
    }

    /// A strip shop's state before the key: the strip (`part` none) or its
    /// list.
    fn strip_state(
        b: &screens::PartsBrowser,
    ) -> Option<(nfsu2::audio::fe_rules::StripShop, nfsu2::audio::fe_rules::StripState)> {
        let shop = Self::strip_shop(b.package)?;
        let c = b.categories.get(b.category);
        let s = match b.part {
            Some(i) => {
                let n = c.map_or(0, |c| c.parts.len());
                nfsu2::audio::fe_rules::StripState {
                    in_list: true,
                    first: i == 0,
                    last: i + 1 >= n,
                    locked: false,
                    opens_list: false,
                }
            }
            None => nfsu2::audio::fe_rules::StripState {
                in_list: false,
                first: b.category == 0,
                last: b.category + 1 >= b.categories.len(),
                locked: c.is_some_and(|c| c.locked),
                opens_list: c.is_some_and(|c| c.action == screens::CategoryAction::Parts && !c.parts.is_empty()),
            },
        };
        Some((shop, s))
    }

    /// The FEng sound notification's extra sound on the screens whose
    /// filter answers (`fe_rules::notify`, sound round 8): the engage
    /// dialogs' Accept / Back, UI_Options' Player category id 11.
    pub(crate) fn menu_notify_sound(&self, key: Key) -> Option<u8> {
        use nfsu2::audio::fe_rules as r;
        let Mode::Menu(menu) = &self.mode else { return None };
        let now = self.now();
        match menu {
            Menu::Engage(e) => {
                r::engage_notify(key, e.focus.accept_focused(&self.game_data.assets, "UI_EngageRaceDialog", e.opened, now), true)
            }
            Menu::ShopEngage(e) => {
                r::engage_notify(key, e.focus.accept_focused(&self.game_data.assets, "UI_EngageShopDialog", e.opened, now), true)
            }
            Menu::Challenge(e) => {
                let focus = e.focus.accept_focused(&self.game_data.assets, e.package(), e.opened, now);
                // UI_EngageEventDialog's package does not answer
                // PAD_ACCEPT; the retry is UI_EngageShopDialog's.
                r::engage_notify(key, focus, matches!(e.kind, screens::ChallengeDialogKind::Retry))
            }
            Menu::Options(o) if o.category == options::Category::Player && o.pause.is_none() => {
                r::options_player_notify(key)
            }
            Menu::Parts(b) => {
                let (shop, s) = Self::strip_state(b)?;
                r::strip_shop_extra(shop, key, s)
            }
            _ => None,
        }
    }

    /// A character typed on the title screen (0x57B8C0): a code it
    /// completes turns on (`cheats`), with the menu's SELECT sound the
    /// first time (0x579CC0 → 0x51DE70(_, 8)); a bonus code (0x5760B0)
    /// also pays the career in memory at once when its intro has not
    /// played and its bank is empty (`cheats::pay_now`). Before the boot's
    /// profile step that career is the boot's (`boot_cash`).
    pub(crate) fn cheat_char(&mut self, c: char) {
        for m in self.fe.cheat_input.char(&nfsu2::frontend::cheats::ACTIVE, c) {
            if m.new {
                self.fe_sound(fe::event::SELECT, fe::control::GENERIC);
            }
            match self.profile.as_mut() {
                Some(p) => p.cash = nfsu2::frontend::cheats::pay_now(m.bonus, p.intro_played, p.cash),
                None => self.fe.boot_cash = nfsu2::frontend::cheats::pay_now(m.bonus, false, self.fe.boot_cash),
            }
        }
    }

    /// A menu event's sound (0x50D150): the event on the kind of control
    /// the screen is built from.
    pub(crate) fn fe_sound(&mut self, event: u8, control: u8) {
        if let Some(id) = fe::sound(event, control) {
            self.fe_id(id);
        }
    }
}
