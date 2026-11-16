mod car_lot;
mod challenge;
mod dialog;
mod dyno_chart;
mod icons;
mod lot_deal;
mod maps;
mod mc;
mod misc;
pub(crate) mod online;
mod options;
mod paint;
mod parts;
mod parts_step;
mod post_race;
mod quick_race;
mod tuning;

use crate::*;

impl Game {
    pub(crate) fn menu_key(&mut self, key: Key) -> Result<()> {
        let now = self.now();
        if matches!(self.mode, Mode::Menu(Menu::Movie(_))) {
            self.movie_key(key);
            return Ok(());
        }
        if key == Key::Start && self.pause_start()? {
            return Ok(());
        }
        if !self.menu_key_sound(key) {
            return Ok(());
        }
        if matches!(self.mode, Mode::Menu(Menu::SmsMailbox(_) | Menu::SmsViewer(_))) {
            self.sms_key(key);
            return Ok(());
        }
        // UI_BuyPerformanceParts is up while Choose Individual Parts is
        // highlighted (0x5620E0): Up and Down move along its list
        // (0x556030 → 0x5390F0) and Accept goes to the row (0x569EF0).
        if let Mode::Menu(Menu::Icons(m, MenuKind::PerfPackage)) = &self.mode
            && m.selected == 0
            && matches!(key, Key::Up | Key::Down | Key::Accept)
        {
            return self.perf_parts_key(key);
        }
        let key_time = self.now();
        let Mode::Menu(menu) = &mut self.mode else { return Ok(()) };
        match menu {
            Menu::LotDeal(_) | Menu::Movie(_) | Menu::SmsMailbox(_) | Menu::SmsViewer(_) => {}
            Menu::McList(..) => self.mc_list_key(key)?,
            Menu::McMessage(..) => self.mc_message_key(key)?,
            Menu::McMain(..) if self.save_flow.mc_create_then.is_some() => self.mc_main_create_key(key, now)?,
            Menu::McMain(..) => self.mc_main_key(key, now)?,
            Menu::TuneSliders(_) | Menu::TuneGraph(_) if key == Key::Start => {
                self.tuning_start();
            }
            Menu::TuneSliders(..) => self.tune_sliders_key(key)?,
            // Back (0x911AB364 in 0x5654E0) → UI_PerformanceTuning_Master.
            Menu::TuneGraph(..) => self.tune_graph_key(key)?,
            Menu::Icons(..) => self.icons_key(key, now)?,
            // UI_QRCarSelect in Quick Race: Accept (0x0C407210, 0x4FC2E5)
            // races in the chosen car, after the transmission question
            // (0x497D50) when the player asks for it; Back returns to the
            // track list, or the modes for Free Run.
            // Customize's car select: Accept (0x4FC2E5) checks for a free
            // "my cars" slot (0x5165E0, else 0x91921A8D "over the limit")
            // and opens UI_ChooseCustomizeCategory (0x552CD0(3, …)) on the
            // car. A slot is free when one of the garage 0x83AD90's 20
            // car entries (+0x544, 0x434 each) has +0x54C zero (the
            // clone's My Cars records, `customize_car_select`). The car is customized on an unsaved
            // profile of its own, which lists parts by the Quick Race rule
            // (0x50F0D0 flag 1, `Profile::part_listed`) and shows no prices.
            Menu::CarLot(lot) if lot.state == LotState::Customize => self.car_lot_customize_key(key, now)?,
            Menu::CarLot(lot) if matches!(lot.state, LotState::QuickRace(_)) => self.car_lot_quick_race_key(key, now)?,
            Menu::CarLot(..) => self.car_lot_key(key, now)?,
            Menu::Status(..) => self.status_key(key)?,
            Menu::InGameMap(..) => self.in_game_map_key(key)?,
            Menu::WorldMap(..) => self.world_map_key(key, key_time)?,
            Menu::QuickRace(..) => self.quick_race_key(key, now)?,
            Menu::QrOptions(..) => self.qr_options_key(key, now)?,
            // 0x499C00: left / right move the focus, accept acts on it
            // (`DialogFocus`).
            Menu::Engage(..) => self.engage_key(key, now)?,
            Menu::SponsorPopup(..) => self.sponsor_popup_key(key)?,
            // UG_LS_Splash (0x4C5580): Accept or Start once the prompt is
            // prov: open game.menu_key-4
            // up go on to UI_Main (MC_Background **open**).
            Menu::Splash(..) => self.splash_key(key)?,
            // UI_OLISPConnect: Accept or Back (0x0C407210 / 0x406415E3 →
            // 0x4A0140) leave the online flow for UI_Main.
            Menu::IspConnect(..) => self.isp_connect_key(key)?,
            // UI_OLEALogin (0x4F0060) and its connect dialog (`EaLogin`).
            Menu::EaLogin(..) => self.ea_login_key(key, now)?,
            // The search dialog's Cancel (0x7345710F) closes it (0x4FB6E0
            // clears the search timer); Back from the list goes to UI_Main
            // (0x4FB740: the network closed, 0x5031B0(0)).
            Menu::LanServers(..) => self.lan_servers_key(key)?,
            // Credits (0x4B87B0): Back → UI_OptionsMain.
            Menu::Credits(..) => self.credits_key(key)?,
            Menu::Controls(..) => self.controls_key(key, now)?,
            // Select buys with the rows' brands (0xE1FDE1D1 after the
            // 0xDE6EFF34 animation; 0x55C940: 0x521F00 writes them to the
            // slots in row order). Back does nothing (Wine, twice; the
            // help bar has no Back).
            Menu::BrandSelect(..) => self.brand_select_key(key, now)?,
            Menu::Jukebox(..) => self.jukebox_key(key, now)?,
            // 0x54A810: at the sweep's end the run is recorded (0x523340);
            // Accept (0x406415E3) records it then if it is still running,
            // ends the run (0x56E690) and plays 0xDE6EFF34 on 0x47FF4E7C,
            // whose end (0xE1FDE1D1) opens UI_PerformanceDyno_Results
            // (0x5377B0). Back does nothing: the handler (0x54A810) takes
            // only 0x406415E3, 0xC98356BA (which re-installs the joystick
            // handlers, 0x52CDE0, and drops the help bar) and 0xE1FDE1D1,
            // with no base handler, so the way out is the results.
            Menu::DynoChart(..) => self.dyno_chart_key(key, now)?,
            // 0x53D520: Accept / Back (0x406415E3 / 0x911AB364) play
            // 0xDE6EFF34; then Accept switches to UI_PerformanceDyno_MAIN
            // (0x7FA21C) and Back to UI_PerformanceDyno_Chart (0x5377B0 /
            // 0x537990, 0x7FA230). The chart it reopens shows the finished
            // pull: the stopped manager (state 0) keeps the last mean's
            // progress and no new run is started.
            Menu::DynoResults(..) => self.dyno_results_key(key, now)?,
            // Back to the covers on this one (0x565B60 → UI_MagazineSelect).
            // UI_MagazineReward (0x55B270): Accept (0x406415E3, or a click
            // on the button 0x69BA0838) asks (0x55B230, GenericDialog) "Do
            // you want to save this magazine?" (0x03312F33; "… DVD?"
            // 0xE9641A65 unless 0x838578 is 1). Yes (0x11223344) takes the
            // screenshot three ticks later (0x4889F0, 320 × 432) and leaves;
            // No (0x11223345) leaves (0x54B0A0: layers freed, 0x838578 = 0,
            // the city again, 0x5404B0). The clone composes covers when
            // they are viewed, so both just leave.
            Menu::Cover(view) if view.reward => self.cover_reward_key(key)?,
            Menu::Cover(..) => self.cover_key(key)?,
            Menu::Stats(..) => self.stats_key(key, now)?,
            Menu::Results(..) => self.results_key(key, now)?,
            Menu::Paint(..) => self.paint_key(key, now)?,
            Menu::Parts(..) => self.parts_key(key, now)?,
            // A world challenge's dialogs (`DialogFocus` as the race
            // dialog). UI_EngageEventDialog's handler 0x498F00: Accept
            // (0xD72F002A) starts the challenge (0x541130) and returns to
            // the game (0x5F4250(1)); Cancel (0x63857DE0) and Back return.
            // The retry (UI_EngageShopDialog's 0x499690 with the failed
            // flag): Accept retries (0x5150D0), Cancel and Back give up
            // (0x501640).
            // A milestone's dialog (0x498F00 with marker +0x0C): only its
            // one button's Accept (0xB8A7C6CE → 0x5010E0, 0x498E70); Back
            // and Left / Right do nothing.
            Menu::Challenge(e) if matches!(e.kind, screens::ChallengeDialogKind::Milestone { .. }) => {
                self.challenge_milestone_key(key)?
            }
            Menu::Challenge(e) if matches!(key, Key::Left | Key::Right) => e.focus.0 = Some((key == Key::Left, now)),
            Menu::Challenge(..) if matches!(key, Key::Accept | Key::Back) => self.challenge_answer_key(key, now)?,
            Menu::Challenge(_) => {}
            // UI_ShowCase_Preview (0x4CF170 → 0x4CEA70 / 0x4CEDC0): the
            // picture state's Accept asks to save; Yes (0x11223344, after
            // its screenshot) and No (0x11223345) both leave (0x499FA0).
            Menu::Showcase(..) => self.showcase_preview_key(key, now)?,
            // UI_ChooseCustomHUD's colour steps (0x5644E0).
            Menu::ChooseHud(..) => self.choose_hud_key(key, now)?,
            // 0x499690: as the race dialog (`DialogFocus`).
            Menu::ShopEngage(e) if matches!(key, Key::Left | Key::Right) => e.focus.0 = Some((key == Key::Left, now)),
            Menu::ShopEngage(e)
                if key == Key::Accept
                    && e.focus.accept_focused(&self.game_data.assets, "UI_EngageShopDialog", e.opened, now) != Some(true) =>
            {
                if e.focus.accept_focused(&self.game_data.assets, "UI_EngageShopDialog", e.opened, now) == Some(false) {
                    self.back_to_city()?;
                }
            }
            Menu::ShopEngage(..) => self.shop_engage_key(key)?,
            Menu::Reward(..) => self.reward_key(key, now)?,
            Menu::Options(..) => self.options_key(key, now)?,
            // Back from the transmission question sends 0x1FAB5998, which
            // the car select ignores: the dialog closes on the car select.
            Menu::Dialog(..) if key == Key::Back && self.shop.ask_gearbox.is_some() => self.dialog_gearbox_back_key()?,
            Menu::Dialog(..) => self.dialog_key(key, now)?,
        }
        Ok(())
    }

    /// The key's menu sound: the native handler's and the FEng notification's, or the
    /// generic one for the screen's control. False for a key no menu reacts to, which
    /// passes silently.
    fn menu_key_sound(&mut self, key: Key) -> bool {
        // The FEng notification's sound (when it differs from the
        // handler's: the same id started twice in one frame is one).
        let notified = self.menu_notify_sound(key);
        if let Some(id) = self.native_menu_sound(key) {
            if let Some(id) = id {
                self.fe_id(id);
            }
            if let Some(n) = notified.filter(|&n| Some(n) != id) {
                self.fe_id(n);
            }
        } else if let Mode::Menu(menu) = &self.mode {
            use fe::{control, event};
            let control = match menu {
                Menu::Icons(..) => control::ICONSCROLLER,
                Menu::CarLot(_) => control::CARLOT,
                Menu::Parts(_) | Menu::ChooseHud(_) => control::BROWSER,
                Menu::Paint(_) => control::PAINT,
                Menu::Dialog(..)
                | Menu::Engage(_)
                | Menu::ShopEngage(_)
                | Menu::Challenge(_)
                | Menu::SponsorPopup(_) => control::BUTTON,
                _ => control::GENERIC,
            };
            let event = match key {
                Key::Left => event::LEFT,
                Key::Right => event::RIGHT,
                Key::Up => event::UP,
                Key::Down => event::DOWN,
                Key::Accept => event::SELECT,
                Key::Back => event::BACK,
                Key::LTrigger => event::LTRIGGER,
                Key::RTrigger => event::RTRIGGER,
                // The graphics shop's Cancel Changes button: its sound
                // event is not traced.
                Key::Button1 => event::SELECT,
                // No menu reacts to these five yet (formats.md, the PC keys):
                // their menu effects are not traced, so they pass silently.
                Key::Start | Key::Button0 | Key::Button3 | Key::Quit => return false,
            };
            // The icon strips play only what their handler (0x543D50)
            // picks for the key, on the column its +0x30 names (the
            // scroller's row of the table equals the generic one).
            let (event, control) = match menu {
                Menu::Icons(m, _) => (m.sound_event(key), control),
                // UI_GenericParts_Browser's handler (0x5664A0): Accept on a
                // locked item (+0x58 → +0x22) plays WRONG, column 0
                // (0x8381E1), and does nothing else.
                Menu::Parts(b)
                    if key == Key::Accept
                        && b.part.is_none()
                        && b.categories.get(b.category).is_some_and(|c| c.locked) =>
                {
                    (Some(event::WRONG), control::GENERIC)
                }
                _ => (Some(event), control),
            };
            if let Some(event) = event {
                self.fe_sound(event, control);
            }
        }
        true
    }

    /// UI_BuyPerformanceParts' list while Choose Individual Parts is highlighted.
    fn perf_parts_key(&mut self, key: Key) -> Result<()> {
        if key == Key::Accept {
            self.mode = self.perf_part_accept();
            return Ok(());
        }
        if let Some(list) = self.shop.perf_parts.as_mut() {
            list.key(key);
            let (at, top) = (list.selected, list.top);
            let mut list = self.perf_parts_list(at);
            list.top = top;
            list.keep_in_view();
            self.shop.perf_parts = Some(list);
        }
        // The package screen's bars follow the highlighted part
        // (0x562540 sends 0xC516CD37 → 0x54FFA0).
        let preview =
            self.shop.perf_parts.as_ref().and_then(|l| l.row()).map(|r| self.perf_ratings(&self.perf_with(r.id)));
        let current = self.perf_ratings(&self.perf_fitted());
        if let Mode::Menu(Menu::Icons(m, _)) = &mut self.mode {
            let preview = preview.unwrap_or(current);
            for (k, (_, values)) in m.bars.iter_mut().enumerate() {
                let pick = |r: &nfsu2::performance::Ratings| [r.acceleration, r.top_speed, r.handling][k];
                if let Some(v) = values.get_mut(0) {
                    *v = (pick(&current), pick(&preview));
                }
            }
        }
        Ok(())
    }
}
