use crate::*;

impl Game {
    /// The offered photo shoots and where they wait.
    pub(crate) fn shoots(&self) -> Vec<(u32, career::Trigger)> {
        let Some(p) = self.profile.as_ref() else { return Vec::new() };
        let mut spots = self.sim.spots.borrow_mut();
        p.offered
            .iter()
            .filter_map(|h| self.game_data.career.milestones.iter().find(|m| m.hash == *h))
            .filter_map(|m| {
                spots.entry(m.spot).or_insert_with(|| self.sim.route_triggers.find(m.spot)).map(|t| (m.hash, t))
            })
            .collect()
    }

    /// Each open event's marker trigger (event +0x28), found again when the
    /// open events change.
    pub(crate) fn refresh_event_triggers(&mut self) {
        let open = self.profile.as_ref().map_or_else(Vec::new, |p| p.open.clone());
        if self.career_world.event_starts.as_ref().is_none_or(|(o, _)| *o != open) {
            let starts = self
                .profile
                .iter()
                .flat_map(|p| p.open_events(&self.game_data.career))
                .filter_map(|e| self.sim.route_triggers.find(e.marker).map(|t| (e.index, t)))
                .collect();
            self.career_world.event_starts = Some((open, starts));
        }
    }

    /// The open shops in the career and where they stand (0x533320): for
    /// each kind 13–15 route trigger, the first shop naming it (+0x3C),
    /// when open (0x513110).
    pub(crate) fn open_shops(&self) -> Vec<(usize, career::Trigger)> {
        let Some(p) = self.profile.as_ref() else { return Vec::new() };
        self.sim.route_triggers
            .iter()
            .filter_map(|t| {
                let s = self.game_data.career.shops.iter().position(|s| s.trigger == t.name)?;
                p.shop_open(&self.game_data.career, &self.game_data.career.shops[s]).then_some((s, *t))
            })
            .collect()
    }

    /// The marker a route trigger stands for (0x527810): an open event
    /// whose marker it is (0x527790), else the first shop naming it
    /// (0x4FFE60), else the first milestone whose spot it is (0x4FFFC0).
    pub(crate) fn marker_target(&self, name: u32) -> Option<MarkerTarget> {
        let p = self.profile.as_ref()?;
        if let Some(e) = self.game_data.career.events.iter().find(|e| e.marker == name && p.open.contains(&e.unlock)) {
            return Some(MarkerTarget::Event(e.index));
        }
        if let Some(s) = self.game_data.career.shops.iter().position(|s| s.trigger == name) {
            return Some(MarkerTarget::Shop(s));
        }
        if let Some(m) = self.game_data.career.milestones.iter().find(|m| m.spot == name) {
            return Some(MarkerTarget::Shoot(m.hash));
        }
        // 0x512A10: the first world challenge whose +0x02 names it.
        self.game_data.career.mag_triggers.iter().position(|t| t.marker == name).map(MarkerTarget::Challenge)
    }

    /// Whether a marker can be engaged (0x527900): an event still open
    /// (0x5007E0), a shop open (0x513110), a milestone on offer (0x501150).
    pub(crate) fn marker_open(&self, m: MarkerTarget) -> bool {
        let Some(p) = self.profile.as_ref() else { return false };
        match m {
            MarkerTarget::Event(e) => p.open.contains(&self.game_data.career.events[e].unlock),
            MarkerTarget::Shop(s) => p.shop_open(&self.game_data.career, &self.game_data.career.shops[s]),
            MarkerTarget::Shoot(h) => p.offered.contains(&h),
            // 0x5135A0: in the list of challenges on offer.
            MarkerTarget::Challenge(i) => self.challenges_on_offer().contains(&i),
        }
    }

    /// Whether entering a marker makes it current (0x532A60): with a world
    /// challenge running (career +0xC08 = 0x85B948, the record 0x533BF0
    /// starts; none in the clone) only a shop or milestone that is its
    /// goal (+0xC), else as `marker_open`. (+0xC08 is not the D-Day
    /// intro: during D-Day the car lot and the garage are ordinary shop
    /// markers, engaged with the key.)
    pub(crate) fn marker_valid(&self, m: MarkerTarget) -> bool {
        if self.career_world.challenge.is_some() {
            return self.challenge_goal(m);
        }
        self.marker_open(m)
    }

    /// The car enters a marker's trigger (0x5EAFC0): a shop is visited
    /// first (0x512FF0; an announced shop found so opens, and the game
    /// puts it on the marker table at once, 0x500E20 → 0x4ECCF0); a valid
    /// marker (0x532A60) becomes current (0x532B20), with the HUD's
    /// prompt for it (0x4CA020 → 0x4C9170, not in the clone). A running
    /// world challenge's goal shop engages at once (0x514FD0 → state 0x11)
    /// and its goal photo spot stops its clock (0x532B20, state 3).
    pub(crate) fn enter_marker(&mut self, m: MarkerTarget) {
        if let (MarkerTarget::Shop(s), Some(p)) = (m, self.profile.as_mut())
            && p.visit_shop(&self.game_data.career.shops[s])
        {
            let _ = p.commit();
        }
        if !self.marker_valid(m) {
            return;
        }
        self.career_world.marker = Some(m);
        // 0x532B20 → 0x5011C0 for a milestone: its spot entered for the
        // first time (visit entry +5 bit 0 clear) with no engage under way
        // (0x861E78) sets the bit and engages at once (0x500B00, state
        // 0x11 → UI_EngageEventDialog; seen under Wine: the magazine's
        // dialog came up on arriving at the spot, no key pressed).
        let auto = matches!(m, MarkerTarget::Shoot(_))
            && matches!(self.mode, Mode::Explore)
            && match (m, self.profile.as_mut()) {
                (MarkerTarget::Shoot(h), Some(p)) => {
                    p.touch(h);
                    let new = p.spotted.insert(h);
                    if new {
                        let _ = p.commit();
                    }
                    new
                }
                _ => false,
            };
        // 0x532B20 with a world challenge running: its goal milestone's
        // spot sets state 3 (the clock stops; the dialog comes by 0x5011C0
        // above or the key); its goal shop, open (0x513110), engages
        // at once (0x514FD0: state 2, the failed flag cleared, the marker
        // copied to +0x8704…, flow event 0x11 → 0x498D00 →
        // UI_EngageShopDialog).
        if let Some(c) = self.career_world.challenge.as_mut() {
            match m {
                MarkerTarget::Shoot(_) => c.state = 3,
                MarkerTarget::Shop(shop)
                    if self.profile.as_ref().is_some_and(|p| p.shop_open(&self.game_data.career, &self.game_data.career.shops[shop])) =>
                {
                    c.state = 2;
                    c.failed = false;
                    self.mode = Mode::Menu(Menu::ShopEngage(ShopEngage {
                        shop,
                        opened: self.now(),
                        focus: Default::default(),
                    }));
                }
                _ => {}
            }
        }
        if auto {
            self.engage();
        }
    }

    /// The engage prompt's kind for a marker (0x4CA020 → 0x4C9170): an
    /// event's by 0x4ABF70 (a URL race 7, else an SUV race 8, else by
    /// type: circuit and special 1, sprint 4, drag 2, drift 3, street X
    /// 5), a shop's by 0x4ABFE0 (+0x50: garage 0xC, graphics 0xF, body
    /// 0xD, performance 0xE, car lot 0xB, specialties 0x10, other 1), a
    /// photo shoot 0x11.
    pub(crate) fn prompt_kind(&self, m: MarkerTarget) -> u8 {
        match m {
            MarkerTarget::Event(e) => {
                let e = &self.game_data.career.events[e];
                if e.category == career::Category::Url {
                    7
                } else if e.suv {
                    8
                } else {
                    match e.kind {
                        career::EventKind::Sprint => 4,
                        career::EventKind::Drag => 2,
                        career::EventKind::Drift => 3,
                        career::EventKind::StreetX => 5,
                        _ => 1,
                    }
                }
            }
            MarkerTarget::Shop(s) => match self.game_data.career.shops[s].kind {
                0 => 0xC,
                1 => 0xF,
                2 => 0xD,
                3 => 0xE,
                4 => 0xB,
                5 => 0x10,
                _ => 1,
            },
            MarkerTarget::Shoot(_) => 0x11,
            MarkerTarget::Challenge(_) => 0x12,
        }
    }

    /// The marker effects (0x605790, each frame out of a race): each kind-0
    /// effect takes the nearest marker table entry (0x500EE0, a career's
    /// only) of its type (0x500DE0) less than 200 m away in x and y,
    /// nearer than the one before by a strictly smaller square
    /// (0x5F0CA0), and stands at its trigger (+4, +8, +0x14). The table
    /// (0x533320) holds, per route trigger in order, an open shop (types
    /// 9–14 by kind, 0x500340: no effect) and an offered photo shoot (6),
    /// then the open events (0x5002E0: 7 an SUV race, else 5 a URL race,
    /// else 4 sprint, 2 street X, 1 drag, 3 drift, 0 the rest); the world
    /// challenges (15) and the DVD id 8 are never in the clone's.
    /// In a race the gate (race type 0x89E7B0 non-zero) turns all 12 off
    /// every world update: `particle_effects`.
    /// The showcase switches them off and on (0x501290, `markers_off`):
    /// its leave turns the update back on, and back in free roam this
    /// places them again.
    pub(crate) fn marker_effects(&mut self) {
        let mut wanted: [Option<(Vec3, f32)>; 12] = [None; 12];
        if let (Some(p), Some(d), false) = (self.profile.as_ref(), self.sim.drive.as_ref(), self.race_flow.quick_free_run) {
            let player = d.state.position.truncate();
            let running = self.career_world.challenge.is_some();
            let shoots = self.sim.route_triggers.iter().filter(|_| !running).filter_map(|t| {
                let m = self.game_data.career.milestones.iter().find(|m| m.spot == t.name)?;
                p.offered.contains(&m.hash).then_some((6, t.position))
            });
            // While a challenge runs, its goal alone (0x515060): a shop
            // has no effect, a photo spot type 6.
            let goal = self
                .challenge_goal_marker()
                .and_then(|(m, t)| matches!(m, MarkerTarget::Shoot(_)).then_some((6, t.position)));
            let challenges = self.challenge_markers().into_iter().map(|(_, t)| (15, t.position));
            let events = p.open_events(&self.game_data.career).into_iter().filter(|_| !running).filter_map(|e| {
                let kind = if e.suv {
                    7
                } else if e.category == career::Category::Url {
                    5
                } else {
                    match e.kind {
                        career::EventKind::Sprint => 4,
                        career::EventKind::StreetX => 2,
                        career::EventKind::Drag => 1,
                        career::EventKind::Drift => 3,
                        _ => 0,
                    }
                };
                self.sim.route_triggers.find(e.marker).map(|t| (kind, t.position))
            });
            // Kind 1: the engage tips still unsent (career +0x600, 0x528990)
            // and kind 2: the cash pickups still open (career +0x800,
            // 0x5141B0), each in route-trigger order; 0x533320 lists
            // neither while a world challenge runs.
            let tips = self
                .sim.route_triggers
                .tips()
                .filter(|_| !running)
                .filter(|t| p.tip_pending(&self.game_data.career, t.name))
                .map(|t| (1, 0, t.position));
            let banks = self
                .sim.route_triggers
                .banks()
                .filter(|_| !running)
                .filter(|t| {
                    self.game_data.career.banks.iter().find(|b| b.trigger == t.name).is_some_and(|b| p.bank_open(&self.game_data.career, b))
                })
                .map(|t| (2, 0, t.position));
            let markers = shoots.chain(goal).chain(events).chain(challenges).map(|(kind, at)| (0, kind, at));
            for (cat, kind, at) in markers.chain(tips).chain(banks) {
                let Some(k) = MARKER_EFFECTS.iter().position(|&(c, id, _)| c == cat && id == kind) else { continue };
                let d2 = (at.truncate() - player).length_squared();
                if d2 < 40000.0 && wanted[k].is_none_or(|(_, best)| d2 < best) {
                    wanted[k] = Some((at, d2));
                }
            }
        }
        let Some(d) = self.sim.drive.as_mut() else { return };
        let world = &d.world;
        d.effects.set_markers(&world.emitters, wanted.map(|w| w.map(|(at, _)| at)), |at| {
            // The ground under the spot by a query of its own (0x5E72F0
            // with no car).
            world.ground_for(&mut Default::default(), Vec3::new(at.x, at.y, 0.0)).map(|(z, _, _)| z)
        });
    }

    /// The showcase's marker switch, off (0x501290(1), from its
    /// constructor 0x4CE770 at 0x4CE9C0): 0x6022B0 frees all 12 marker
    /// slots (0x602230 each, their particles with them) and clears the
    /// update's flag 0x8022F8, so 0x605790 places none (0x6057A9) until
    /// the leave (0x499FA0 → 0x501290(0) at 0x49A000 → 0x5F0C90) sets it
    /// again. The clone places them only from the free-roam update, which
    /// does not run under the showcase: back in free roam
    /// `marker_effects` puts them back.
    pub(crate) fn markers_off(&mut self) {
        if let Some(d) = self.sim.drive.as_mut() {
            d.effects.set_markers(&d.world.emitters, [None; 12], |_| None);
        }
    }

    /// The Engage key ("Enter Race / Shop", DIACTION 15, code 0xBF in
    /// 0x602BE0): with the car in a marker's trigger (player +0x9AC) whose
    /// marker can still be engaged (0x527900), the engage dialog (state
    /// 0x11 → 0x498D00): UI_EngageRaceDialog for an event,
    /// UI_EngageShopDialog for a shop, UI_EngageEventDialog for a photo
    /// shoot (not in the clone: it takes the shot, 0x533690, and shows the
    /// cover's title).
    pub(crate) fn engage(&mut self) -> bool {
        let Some(m) = self.career_world.marker.filter(|&m| self.career_world.marker_trigger.is_some() && self.marker_open(m)) else {
            return false;
        };
        let opened = self.now();
        self.mode = match m {
            MarkerTarget::Event(event) => {
                // 0x4B551B → 0x512620: the message number drawn once
                // from the game's generator (0x43C1C0(n) + 1; the clone's
                // (docs/original/rng.md: on the shared generator, the clone's career
                // generator is gone), none for stage 0.
                let message = match (screens::message_set(&self.game_data.career.events[event]), self.profile.as_ref()) {
                    (Some((_, n)), Some(_)) => self.rng.below(n) + 1,
                    _ => 1,
                };
                // The dialog loads the race's map, TRACKMAP<track> (0x4B5491).
                self.game_data.assets.track_map(&self.game_data.root, self.game_data.career.events[event].track);
                let e = &self.game_data.career.events[event];
                let purse = self.profile.as_ref().map_or(e.reward, |p| p.race_bank(&self.game_data.career, e));
                Mode::Menu(Menu::Engage(Engage { event, message, opened, focus: Default::default(), purse }))
            }
            MarkerTarget::Shop(shop) => {
                Mode::Menu(Menu::ShopEngage(ShopEngage { shop, opened, focus: Default::default() }))
            }
            MarkerTarget::Challenge(challenge) => {
                // UI_EngageEventDialog (0x4991B0): 0x515160 draws its
                // message number from the game's generator.
                let kind = self.game_data.career.mag_triggers.get(challenge).map_or(0, |t| t.kind);
                let message = match (screens::challenge_message_range(kind), self.profile.as_ref()) {
                    (Some(n), Some(_)) => self.rng.below(n) + 1,
                    _ => 1,
                };
                Mode::Menu(Menu::Challenge(screens::ChallengeDialog {
                    kind: screens::ChallengeDialogKind::Offer { challenge, message },
                    opened,
                    focus: Default::default(),
                    from: self.sender_name(),
                }))
            }
            // A milestone: UI_EngageEventDialog (0x4991B0's milestone
            // branch, seen under Wine); its Continue runs the shoot
            // (`shoot_accept`).
            MarkerTarget::Shoot(milestone) => Mode::Menu(Menu::Challenge(screens::ChallengeDialog {
                kind: screens::ChallengeDialogKind::Milestone { milestone },
                opened,
                focus: Default::default(),
                from: self.sender_name(),
            })),
        };
        true
    }

    /// UI_EngageEventDialog's sender (0x499160): one name drawn as the
    /// opponents' are (0x4FE970(&name, 1): Rand_Int(53) of the table
    /// 0x7F6CD0), shown as "From: %s" (0x9F29D70D) on 0xFECED617. Seen
    /// under Wine: "From: Lola" for a challenge, "From: Carol" for a DVD.
    pub(crate) fn sender_name(&mut self) -> String {
        let i = if self.profile.is_some() { self.rng.below(53) as usize } else { 0 };
        nfsu2::frontend::quick_race::NAMES[i].to_string()
    }

    /// The milestone dialog's Continue (0x498F00 on 0xB8A7C6CE → 0x5010E0,
    /// 0x498E70): the photo shoot of milestone `h`.
    pub(crate) fn shoot_accept(&mut self, h: u32) {
        let rating = self.visual_rating();
        // The milestone's engage (0x498E70 / the showcase 0x4CE770):
        // the reward (0x541940 → 0x541220: cash, then the cover at
        // 0x837FD8), the running challenge done if in state 3
        // (0x541970), the shoot done (0x533690).
        let Some(p) = self.profile.as_mut() else { return };
        let cash = p.cover_reward(&self.game_data.career, h, rating);
        // 0x541220 → 0x4AC170 (with the HUD's cash element): the
        // popup armed for four HUD updates.
        let kind = self.game_data.career.milestones.iter().find(|x| x.hash == h).map_or(0, |x| x.kind);
        self.hud_state.cash_popup = Some((kind, cash, 4));
        let Some(p) = self.profile.as_mut() else { return };
        p.shoot(&self.game_data.career, h, &mut self.rng);
        if self.career_world.challenge.is_some_and(|c| c.state == 3) {
            self.finish_challenge();
        }
        let Some(p) = self.profile.as_mut() else { return };
        let _ = p.commit();
        // The dialog's accept (0x498E70): a milestone of kind 3 or
        // 4 (0x5011A0) takes the shot (0x533690) and opens
        // UI_MagazineReward in the front end (0x50C8D0, state 4);
        // kinds 1 and 2 open UI_ShowCase_Preview (0x537990; Control
        // Car, Show Off, Cycle Cameras, Take Picture).
        let then = match self.game_data.career.milestones.iter().position(|x| x.hash == h) {
            Some(i) if matches!(self.game_data.career.milestones[i].kind, 3 | 4) => {
                // 0x498ECA → 0x500AB0, before the front end: the car's
                // place kept, career +0x8744 = 1 (the city start after
                // the reward, and a save made there, put the car back).
                if let Some(at) = self.here()
                    && let Some(p) = self.profile.as_mut()
                {
                    p.keep_explore_at(at);
                    let _ = p.commit();
                }
                self.magazine_reward(i)
            }
            Some(i) if matches!(self.game_data.career.milestones[i].kind, 1 | 2) => {
                Mode::Menu(Menu::Showcase(self.showcase_preview(i, self.game_data.career.milestones[i].kind == 2, self.now())))
            }
            _ => Mode::Explore,
        };
        self.mode = then;
    }
}
