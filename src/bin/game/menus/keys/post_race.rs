use crate::*;

impl Game {
    pub(super) fn cover_reward_key(&mut self, key: Key) -> Result<()> {
        let Mode::Menu(Menu::Cover(view)) = &mut self.mode else { unreachable!() };
        if key == Key::Accept {
            let (milestone, dvd) = (view.milestone, view.dvd);
            let text = if dvd {
                self.label(0xE964_1A65, "Do you want to save this DVD?")
            } else {
                self.label(0x0331_2F33, "Do you want to save this magazine?")
            };
            let (yes, no) = self.yes_no();
            // Yes writes the cover as a picture save ("<name> DVD
            // <n>" beside the profile, `LotDeal::SaveCover`) and
            // tells "Save complete." with Continue before free
            // roam: **observed** under Wine (the stage-2
            // DVD shoot).
            let saved = Mode::Menu(Menu::LotDeal(LotDeal::SaveCover { milestone, dvd }));
            self.mode = Mode::Menu(Menu::Dialog(
                Dialog::new("GenericDialog_SMALL", &text, &[&yes, &no], self.now()),
                vec![saved, Mode::Explore],
            ));
        }
        Ok(())
    }

    pub(super) fn cover_key(&mut self, key: Key) -> Result<()> {
        let Mode::Menu(Menu::Cover(view)) = &mut self.mode else { unreachable!() };
        if matches!(key, Key::Back | Key::Accept) {
            let (dvd, at) = (view.dvd, view.milestone);
            let selected = self.earned_covers(dvd).iter().position(|&i| i == at).unwrap_or(0);
            self.mode = self.covers_menu(dvd, selected);
        }
        Ok(())
    }

    pub(super) fn stats_key(&mut self, key: Key, now: f32) -> Result<()> {
        let Mode::Menu(Menu::Stats(stats)) = &mut self.mode else { unreachable!() };
        match key {
            Key::Back => self.mode = self.icon_menu(MenuKind::PostRace),
            k => stats.key(k, now),
        }
        Ok(())
    }

    pub(super) fn results_key(&mut self, key: Key, now: f32) -> Result<()> {
        let Mode::Menu(Menu::Results(results, reward)) = &mut self.mode else { unreachable!() };
        // The standings' Back (0x4EA6CA → 0x4C0020): the race's
        // results again. The results themselves have no Back (0x4E9A20).
        if key == Key::Back {
            if matches!(results.kind, ResultsKind::Standings { .. })
                && let Some(ChainStep::Results { .. }) = self.race_flow.chain.last()
            {
                self.chain_back(now);
            }
            return Ok(());
        }
        if key == Key::Accept {
            if results.kind == ResultsKind::Tournament
                && let Some(mut standings) = self.race_flow.standings.take()
            {
                self.race_flow.chain.push(ChainStep::Results {
                    results: results.clone(),
                    reward: reward.clone(),
                    standings: Some(standings.clone()),
                });
                standings.opened = now;
                *results = standings;
                return Ok(());
            }
            // Between a tournament's races the career goes on to
            // UI_PostRace and its "Next Race" (0x4F3EA0 adds the
            // reward only after the last race). A quick race has
            // one race (the mode record's track), so it never
            // gets here.
            if self.race_flow.next_round.is_some() {
                self.race_flow.chain.clear();
                self.mode = self.icon_menu(MenuKind::PostRace);
                return Ok(());
            }
            let step = ChainStep::Results { results: results.clone(), reward: reward.clone(), standings: None };
            self.mode = match reward.take() {
                Some(mut r) => {
                    self.race_flow.chain.push(step);
                    r.opened = now;
                    Mode::Menu(Menu::Reward(r))
                }
                None => {
                    self.race_flow.chain.clear();
                    self.icon_menu(MenuKind::PostRace)
                }
            };
        }
        Ok(())
    }

    /// Back in the post-race chain (0x4C0020): the screen before, as it
    /// was left; on the chain's first screen nothing.
    pub(super) fn chain_back(&mut self, now: f32) {
        let Some(step) = self.race_flow.chain.pop() else { return };
        self.mode = match step {
            ChainStep::Results { mut results, reward, standings } => {
                if standings.is_some() {
                    self.race_flow.standings = standings;
                }
                results.opened = now;
                Mode::Menu(Menu::Results(results, reward))
            }
            ChainStep::Reward(mut r) => {
                r.opened = now;
                Mode::Menu(Menu::Reward(r))
            }
            ChainStep::Popup => Mode::Menu(Menu::SponsorPopup(now)),
        };
    }
}
