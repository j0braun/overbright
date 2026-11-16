use crate::*;

impl Game {
    pub(super) fn challenge_milestone_key(&mut self, key: Key) -> Result<()> {
        let Mode::Menu(Menu::Challenge(e)) = &mut self.mode else { unreachable!() };
        if let (screens::ChallengeDialogKind::Milestone { milestone }, Key::Accept) = (e.kind, key) {
            self.shoot_accept(milestone);
        }
        Ok(())
    }

    pub(super) fn challenge_answer_key(&mut self, key: Key, now: f32) -> Result<()> {
        let Mode::Menu(Menu::Challenge(e)) = &mut self.mode else { unreachable!() };
        let accept =
            key == Key::Accept && e.focus.accept_focused(&self.game_data.assets, e.package(), e.opened, now) == Some(true);
        let cancel =
            key == Key::Back || e.focus.accept_focused(&self.game_data.assets, e.package(), e.opened, now) == Some(false);
        match e.kind {
            screens::ChallengeDialogKind::Offer { challenge, .. } if accept => {
                self.start_challenge(challenge);
                self.explore(self.here())?;
            }
            screens::ChallengeDialogKind::Retry if accept => self.answer_retry(true)?,
            screens::ChallengeDialogKind::Offer { .. } if cancel => self.back_to_city()?,
            screens::ChallengeDialogKind::Retry if cancel => self.answer_retry(false)?,
            _ => {}
        }
        Ok(())
    }

    pub(super) fn showcase_preview_key(&mut self, key: Key, now: f32) -> Result<()> {
        let Mode::Menu(Menu::Showcase(v)) = &mut self.mode else { unreachable!() };
        let k = v.key(key, now);
        let mut v2 = v.clone();
        self.showcase_key(&mut v2, k);
        if let Mode::Menu(Menu::Showcase(v)) = &mut self.mode {
            *v = v2;
        }
        if k == screens::ShowcaseKey::Save
            && let Mode::Menu(Menu::Showcase(v)) = &self.mode
        {
            // Both answers leave (0x499FA0), which shuts what Show
            // Off left open (0x499F10).
            self.showcase.shut = v.open;
            let question = v.question();
            let text = self.label(question, "Do you want to save this magazine?");
            let (yes, no) = self.yes_no();
            let saved = Mode::Menu(Menu::LotDeal(LotDeal::SaveShowcase(v.clone())));
            self.mode = Mode::Menu(Menu::Dialog(
                Dialog::new("GenericDialog_SMALL", &text, &[&yes, &no], self.now()),
                vec![saved, Mode::Explore],
            ));
        }
        Ok(())
    }

    pub(super) fn choose_hud_key(&mut self, key: Key, now: f32) -> Result<()> {
        let Mode::Menu(Menu::ChooseHud(c)) = &mut self.mode else { unreachable!() };
        match c.key(key, now) {
            nfsu2::frontend::custom_hud::ChooseKey::None => {}
            // Back from the backing colour: the gauge list (state 1).
            nfsu2::frontend::custom_hud::ChooseKey::Back => {
                let (category, part) = (c.category, c.part);
                self.mode = self.specialties_browser(category, Some(part));
            }
            // The needle colour accepted: state 0, 0x9E382ECD to the
            // overlay, which buys the gauge and fits the picks.
            nfsu2::frontend::custom_hud::ChooseKey::Done => {
                let c = c.clone();
                self.buy_custom_gauge(&c);
            }
        }
        Ok(())
    }

    pub(super) fn shop_engage_key(&mut self, key: Key) -> Result<()> {
        let Mode::Menu(Menu::ShopEngage(e)) = &mut self.mode else { unreachable!() };
        match key {
            Key::Accept => {
                let index = e.shop;
                let shop = self.game_data.career.shops[index].clone();
                self.shop.visit = Some(shop.hash);
                // 0x499640: the shop's movie (+0x20) plays when it has
                // one and its visit-table entry is not yet in state 5
                // (0x500460) — the first entry (0x499470's argument,
                // kept at 0x836350); 0x54CB00 then sets state 5.
                if let Some(p) = self.profile.as_mut() {
                    self.shop.movie = !shop.movie.is_empty() && !p.entered.contains(&shop.hash);
                    p.touch(shop.hash);
                    p.entered.insert(shop.hash);
                    p.build_shop_ride();
                }
                // 0x54CB00: the running challenge's goal shop entered,
                // state 4 (and 0x533D30 lists the unique parts of its
                // slots, +0x15..+0x17, for the unique-part screen).
                if self.challenge_record().is_some_and(|t| t.milestone == shop.hash)
                    && let Some(c) = self.career_world.challenge.as_mut()
                {
                    c.state = 4;
                }
                // Entering a shop (0x54CB00, 0x54CC0C): the car's place
                // at the trigger is kept, career +0x8744 = 1, so the
                // shop's save point writes it and Resume (0x495A30)
                // starts there: the car lot's first car, a garage visit
                // (the original's own car-lot save: flag 1, the lot's
                // trigger place). Then the intro's goal there wins its
                // event.
                // The game writes the save at its own save points, not
                // here: event A won in the car lot is kept once a car
                // is taken (LotDeal::Take saves), so quitting in the
                // lot leaves no save stuck in B without a car.
                if let Some(at) = self.here()
                    && let Some(p) = self.profile.as_mut()
                {
                    p.keep_explore_at(at);
                }
                if let Some(p) = self.profile.as_mut()
                    && p.enter_shop(&self.game_data.career, &self.game_data.root, shop.hash, &mut self.rng)
                    && !(p.intro == Some(career::DDAY_EVENT_B) && p.car.is_empty())
                {
                    let _ = p.commit();
                }
                // 0x54CB00 → 0x529870: the shop's messages.
                if let Some(p) = self.profile.as_mut() {
                    p.sms_shop_entered(&self.game_data.career, shop.hash);
                }
                // 0x499640 → 0x499470: the drive-in scene, whose end
                // opens the shop (0x4995A0); with none, the shop opens
                // at once.
                if self.start_shop_scene(nfsu2::shop_scene::SceneKind::Enter, index) {
                    self.mode = Mode::Explore;
                } else {
                    self.mode = self.open_shop(index);
                }
            }
            Key::Back => {
                self.back_to_city()?;
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn reward_key(&mut self, key: Key, now: f32) -> Result<()> {
        if matches!(key, Key::Accept | Key::Back) {
            // A sponsor on offer: UI_SponsorPopup, then the
            // sponsorship screen (0x4F3EA0, 0x4A4610).
            self.mode = if self.profile.as_ref().is_some_and(|p| p.needs_sponsor(&self.game_data.career)) {
                // Its INIT (0x4C13DF): 0x525520, Rachel's line.
                let line = self.profile.as_mut().and_then(|p| p.hints.sponsor_popup());
                self.say(line);
                Mode::Menu(Menu::SponsorPopup(now))
            } else {
                self.icon_menu(MenuKind::PostRace)
            };
        }
        Ok(())
    }
}
