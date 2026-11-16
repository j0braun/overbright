use crate::*;

impl Game {
    pub(super) fn car_lot_customize_key(&mut self, key: Key, now: f32) -> Result<()> {
        let Mode::Menu(Menu::CarLot(lot)) = &mut self.mode else { unreachable!() };
        // Back (0x911AB364 → 0x4B22C0): UI_Main, through the save
        // point (0x4A3100 mode 0x42) when a customize session
        // changed the profile (0x838540, then cleared, 0x50E2D0).
        if key == Key::Back {
            if std::mem::take(&mut self.save_flow.customize_changed) {
                self.save_point(false, AfterSave::Menu(MenuKind::Main))?;
            } else {
                self.mode = self.icon_menu(MenuKind::Main);
            }
            return Ok(());
        }
        // Delete Tuned Car (help bit 0x40000; BUTTON1 0xC519BFC0 in
        // game mode 0x20, 0x4FC1F0): a My Cars record (0x5163A0) not
        // in a career slot (0x516340) nor in garage +0x9A8C's 12
        // entries (0x503590) asks "Would you like to delete this
        // customized car?" (0x367161F5; Yes 0xF770 / No 0x061C, No
        // focused); Yes (0x51123592 → 0x4EF090) clears the record
        // (0x52A6D0 as 0x521110 does), the filter becomes All Cars
        // (0x83AAC0 = 3) and the list is rebuilt, on the next car
        // (the previous one when it was the last). Nothing is saved
        // (under Wine: no save question after it, nor on Back).
        if key == Key::Button1 {
            if let Some(k) = lot.cars.get(lot.selected).and_then(|c| c.my_car) {
                let lot = lot.clone();
                let again = Mode::Menu(Menu::CarLot(lot.clone()));
                let text = self.label(0x3671_61F5, "Would you like to delete this customized car?");
                let buttons = [self.label(0xF770, "Yes"), self.label(0x061C, "No")];
                let mut d =
                    Dialog::new(screens::dialog_package(&self.game_data.assets, &text), &text, &[&buttons[0], &buttons[1]], now);
                d.selected = 1;
                let yes = Mode::Menu(Menu::LotDeal(LotDeal::DeleteTuned { record: k, at: lot.selected }));
                self.mode = Mode::Menu(Menu::Dialog(d, vec![yes, again]));
            }
            return Ok(());
        }
        let Some(i) = lot.key(key, now) else { return Ok(()) };
        let choice = lot.cars[i].clone();
        let again = Mode::Menu(Menu::CarLot(lot.clone()));
        self.customize_car_select(choice, again, None);
        Ok(())
    }

    pub(super) fn car_lot_quick_race_key(&mut self, key: Key, now: f32) -> Result<()> {
        let Mode::Menu(Menu::CarLot(lot)) = &mut self.mode else { unreachable!() };
        let LotState::QuickRace(event) = lot.state else { unreachable!() };
        if key == Key::Back {
            self.mode = match (event, self.quick.options.clone()) {
                (Some(_), Some(mut o)) => {
                    o.opened = now;
                    Mode::Menu(Menu::QrOptions(o))
                }
                _ => self.icon_menu(MenuKind::QrMode),
            };
            return Ok(());
        }
        // The triggers switch the filter (0x4EED10), skipping a
        // filter without cars: My Cars without records, Career (the
        // prov: inferred game.menu_key
        // clone keeps no career records here, **inferred**), SPONSOR
        // CARS until a sponsor car's code is typed at the title.
        if matches!(key, Key::LTrigger | Key::RTrigger) {
            let filter =
                nfsu2::frontend::screens::next_qr_filter(lot.filter, key == Key::RTrigger, |f| self.qr_filter_has(f));
            let cars = self.qr_cars(filter);
            self.quick.filter = filter;
            if let Mode::Menu(Menu::CarLot(lot)) = &mut self.mode {
                lot.filter = filter;
                lot.cars = cars;
                lot.selected = 0;
            }
            return Ok(());
        }
        // Customize (BUTTON1 0xC519BFC0 in game mode 2, 0x4FC1F0):
        // the filter becomes All Cars (0x83AAC0 = 3) and the car is
        // customized (0x552CD0(2, …)); its Back comes here again
        // (0x55BE00), through the save point (0x2042) when changed.
        // Not for a sponsor car: the help bar drops Customize for
        // the Sponsor filter (0x4B2310; that the key does nothing
        // prov: inferred game.menu_key-2
        // there is **inferred**).
        if key == Key::Button1 {
            if let Some(choice) = lot.cars.get(lot.selected).filter(|c| c.preset.is_none()).cloned() {
                let back = lot.clone();
                let again = lot_again_qr(&back);
                self.customize_car_select(choice, again, Some(back));
            }
            return Ok(());
        }
        let Some(i) = lot.key(key, now) else { return Ok(()) };
        let lot = lot.clone();
        self.quick.car = Some(lot.cars[i].name.clone());
        // A My Cars record races with its parts (0x502A40 names it
        // player 0's car).
        // A sponsor car races as its preset (0x637280 puts the
        // preset on the player's ride).
        self.quick.parts = match &lot.cars[i].preset {
            Some(name) => self.preset_ride(name),
            None => lot.cars[i]
                .my_car
                .and_then(|k| self.profile.as_ref()?.my_cars.get(k)?.as_ref())
                .map(|m| (m.installed.clone(), m.perf.clone())),
        };
        self.drop_drive();
        // 0x4FC2E5 in game mode 2: outside a drag race (0x83AAB4 ≠
        // 2) the player's Ask for Transmission (0x83AA6A) puts the
        // question first (0x497D50: "Transmission:" 0x30003D27,
        // Manual 0x1B3A2485 / Auto 0x61CB0720).
        let drag = event.is_some_and(|e| self.game_data.career.events[e].kind == career::EventKind::Drag);
        if !drag && self.settings.players[0].ask_transmission {
            let text = self.label(0x3000_3D27, "Transmission:");
            let buttons = [self.label(0x1B3A_2485, "Manual"), self.label(0x61CB_0720, "Auto")];
            // No package named (0x497D67): 0x526C50 sizes it.
            let mut d =
                Dialog::new(screens::dialog_package(&self.game_data.assets, &text), &text, &[&buttons[0], &buttons[1]], now);
            // prov: inferred game.menu_key-3
            // **Inferred**: the current setting's button starts
            // selected (0x5587A0's flag is the setting).
            d.selected = if self.settings.players[0].manual { 0 } else { 1 };
            self.shop.ask_gearbox = Some(event);
            // Over the car select, dimmed (Wine, menus15: the list
            // and the car under the question, StartDim's look).
            self.mode =
                dialog_over(Mode::Menu(Menu::Dialog(d, vec![lot_again_qr(&lot), lot_again_qr(&lot)])), true, None);
            return Ok(());
        }
        match event {
            Some(e) => self.start_race(e, true, false)?,
            None => {
                self.race_flow.quick_free_run = true;
                self.explore(None)?;
            }
        }
        Ok(())
    }

    pub(super) fn car_lot_key(&mut self, key: Key, now: f32) -> Result<()> {
        let Mode::Menu(Menu::CarLot(lot)) = &mut self.mode else { unreachable!() };
        let lot_again = Mode::Menu(Menu::CarLot(lot.clone()));
        if key == Key::Back {
            self.mode = if lot.state == LotState::CribSelect {
                self.icon_menu(MenuKind::Crib)
            } else if self.profile.as_ref().is_some_and(|p| p.slots.first().is_some_and(|s| s.car.is_none())) {
                // 0x911AB364: CAREER_SLOT_0 without a car of its own.
                self.dialog(&self.label(0x272E_BEBA, "Rachel needs her car back."), lot_again)
            } else {
                self.career_world.pending_explore = self.here();
                let mut m = self.ask(0xD76E_0B5A, Mode::Explore, lot_again);
                // No focused (0x5587A0's eighth argument 1 at
                // 0x4EDE90; Wine menus111 carlot_exit shows No lit).
                if let Mode::Menu(Menu::Dialog(d, _)) = &mut m {
                    d.selected = 1;
                }
                // 0x4ED9E0 on the Yes (0x51123592): 0x50D150(10,
                // +0x30), table [BACK][CARLOT].
                if let Mode::Menu(Menu::Dialog(d, _)) = &mut m {
                    d.answer_sound = Some((0, nfsu2::audio::fe_rules::LOT_LEAVE_YES));
                }
                m
            };
            // Both pushed by the lot's Back handler (0x4EDE8B
            // 0x5587A0 / 0x4EDED8 0x558580, parent the lot's
            // package) with no FORWARD: the lot stays under them
            // (Wine, menus107 c24).
            if let Mode::Menu(Menu::Dialog(d, outcomes)) = &mut self.mode {
                d.under = outcomes.iter().position(|m| matches!(m, Mode::Menu(Menu::CarLot(_))));
            }
            return Ok(());
        }
        let Some(i) = lot.key(key, now) else { return Ok(()) };
        let (state, chosen, stock) = (lot.state, lot.cars[i].clone(), lot.stock.clone());
        let Some(p) = self.profile.as_ref() else { return Ok(()) };
        let current = p.car.clone();
        let deal = |slot: usize, car: &str| Mode::Menu(Menu::LotDeal(LotDeal::Take { slot, car: car.into() }));
        let yes_no = [(0xF770, "Yes"), (0x061C, "No")];
        let lot_dim = Mode::Menu(Menu::CarLot(lot.clone()));
        self.mode = match state {
            // 0x4DFC50 on Accept: a locked slot "You'll have to
            // win some more races!" (0x2415BE66), an open one "Pick
            // up your FREE sponsor car^ at the local car lot."
            // (0xCF3B2D70), the car being driven "You're already
            // driving your %s!" (0x7EE84E4E), all with OK; another
            // car "Do you want to drive your %s instead of your
            // %s?" (0x1231C38F), Yes (0xD05FC3A3) selecting it
            // (0x500BD0) and leaving for the crib, No staying.
            LotState::CribSelect => match (chosen.placeholder, chosen.slot) {
                (Some(Placeholder::Locked), _) => {
                    self.dialog(&self.label(0x2415_BE66, "You'll have to win some more races!"), lot_again)
                }
                (Some(Placeholder::Open), _) => self
                    .dialog(&self.label(0xCF3B_2D70, "Pick up your FREE sponsor car at the local car lot."), lot_again),
                (None, Some(slot)) if slot == p.slot => {
                    let text = self.label(0x7EE8_4E4E, "You're already driving your %s!").replacen(
                        "%s",
                        &self.game_data.assets.strings.label(&format!("CAR_NAME_{}", chosen.name)),
                        1,
                    );
                    self.dialog(&text, lot_again)
                }
                (None, Some(slot)) => {
                    let mut m = self.ask_cars(
                        0x1231_C38F,
                        &[&chosen.name, &current],
                        yes_no,
                        [Mode::Menu(Menu::LotDeal(LotDeal::Select { slot })), lot_again],
                    );
                    // 0x4DFC50 on the Yes (0xD05FC3A3): 0x50D150(10,
                    // +0x30) before going back to the crib.
                    if let Mode::Menu(Menu::Dialog(d, _)) = &mut m {
                        d.answer_sound = Some((0, nfsu2::audio::fe_rules::LOT_LEAVE_YES));
                    }
                    m
                }
                (None, None) => lot_again,
            },
            // "Do you want this stock %s?"
            LotState::Take => match p.free_slot() {
                Some(slot) => {
                    self.ask_cars(0xDA8F_3E96, &[&chosen.name], yes_no, [deal(slot, &chosen.name), lot_again])
                }
                None => lot_again,
            },
            // "Do you want to trade-in your %s ^for this stock %s?"
            LotState::TradeIn => {
                self.ask_cars(0x8521_5BC6, &[&current, &chosen.name], yes_no, [deal(p.slot, &chosen.name), lot_again])
            }
            // ...^Or do you want to choose one of your other cars?"
            LotState::TradeOrBrowse => self.ask_cars(
                0x3192_F5D1,
                &[&current, &chosen.name],
                [(0xFB75_E78F, "Trade this car?"), (0x2699_CF8E, "Browse My Other Cars")],
                [deal(p.slot, &chosen.name), Mode::Menu(Menu::LotDeal(LotDeal::Browse { car: chosen.name.clone() }))],
            ),
            // One of the player's cars for the stock one.
            LotState::Browse => match (chosen.slot, stock) {
                (Some(slot), Some(stock)) => {
                    self.ask_cars(0x8521_5BC6, &[&chosen.name, &stock], yes_no, [deal(slot, &stock), lot_again])
                }
                _ => lot_again,
            },
            LotState::QuickRace(_) | LotState::Customize => lot_again,
        };
        // States 2–5 ask through 0x5587A0 (parent the lot's
        // package), then play the lot's DIM (0x4EDCBC): the lot
        // fades out in 100 ms under the question while the 3D car
        // stays (Wine menus107 c14). Trade-or-browse has no lot
        // outcome, so the lot rides as a third outcome no button
        // reaches (Back answers the last button, 1) and fades the
        // same 100 ms.
        if state == LotState::TradeOrBrowse
            && let Mode::Menu(Menu::Dialog(_, outcomes)) = &mut self.mode
        {
            outcomes.push(lot_dim);
        }
        if matches!(state, LotState::Take | LotState::TradeIn | LotState::TradeOrBrowse | LotState::Browse)
            && let Mode::Menu(Menu::Dialog(d, outcomes)) = &mut self.mode
            && let Some(i) = outcomes.iter().position(|m| matches!(m, Mode::Menu(Menu::CarLot(_))))
        {
            d.under = Some(i);
            d.dim_under = Some(("UI_CareerCarLot", screens::DIM_HANDLER));
        }
        Ok(())
    }
}
