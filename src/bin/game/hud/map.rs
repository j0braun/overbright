use crate::*;

impl Game {
    /// UI_InGame_WorldMap for career free roam (see [`InGameWorldMap`]),
    /// or None outside a career or in an outrun (race state 7).
    /// The Career Statistics key (code 0xC1, 0x602BE0): in a career's
    /// free roam with no race, outrun or engage under way it raises state
    /// 0x13, and 0x6068D5 (game state ≥ 4) opens UI_Status_Master, whose
    /// factory fills the statistics first (0x4E1B80 → 0x528A70).
    pub(crate) fn status_screen(&self) -> Option<Mode> {
        use nfsu2::frontend::status::{CareerStats, StatusScreen, career_rows};
        let career = self.profile.is_some() && !self.race_flow.quick_free_run;
        if !career
            || !matches!(self.mode, Mode::Explore)
            || self.sim.encounters.as_ref().is_some_and(|e| e.outrun.is_some())
        {
            return None;
        }
        let stats = CareerStats::new(self.profile.as_ref()?, &self.game_data.career);
        let label = |h: u32| self.game_data.assets.strings.get(h).unwrap_or("").to_string();
        // "Total Spent On Car:": the current slot's +0x7F0 (0x4E1526),
        // which the part purchase 0x547580 adds each part's price to.
        let rows = career_rows(&stats, &label, self.profile.as_ref()?.spent);
        let mut screen = StatusScreen::new(&stats, self.now());
        // The Visual Rating stars (0x53D880, 0x4E153F): the ride of the car
        // the player drives (player 0x8900AC → car +4 → +0x10), so in the
        // D-Day intro, with no car of the player's own, Rachel's
        // DDAY_PLAYER_CAR (Wine round 3A: three and a half stars on a fresh
        // career, the clone showed none).
        screen.rating = Some(
            match self
                .profile
                .as_ref()
                .filter(|p| p.car.is_empty())
                .and(self.game_data.presets.iter().find(|x| x.name == career::DDAY_PLAYER_CAR))
            {
                Some(x) => self.visual_rating_car(&x.car, &x.installed()),
                None => self.visual_rating(),
            },
        );
        Some(Mode::Menu(Menu::Status(Box::new(StatusMenu { screen, stats, rows }))))
    }

    pub(crate) fn in_game_map(&mut self) -> Option<Mode> {
        let career = self.profile.is_some() && !self.race_flow.quick_free_run;
        if !career
            || !matches!(self.mode, Mode::Explore)
            || self.sim.encounters.as_ref().is_some_and(|e| e.outrun.is_some())
        {
            return None;
        }
        // The map takes the track the race state's 0x89E7A0 names: free
        // roam's 4000.
        let texture = self.game_data.assets.track_map(&self.game_data.root, 4000);
        let t = self.game_data.career.tracks.get(&4000)?;
        let frame = [t.map_left, t.map_top, t.map_size];
        let stage = self.profile.as_ref().map_or(0, |p| p.stage);
        let icons = self.map_icons();
        let gps = self.career_world.gps.as_ref().filter(|g| g.on()).map(|g| g.target);
        Some(Mode::Menu(Menu::InGameMap(InGameWorldMap::new(
            self.now(),
            screens::world_map_max_zoom(stage),
            texture,
            frame,
            icons,
            self.profile.as_ref().map_or(self.career_world.map_hidden, |p| p.map_hidden),
            gps,
        ))))
    }

    /// Whether an open event's marker is on the maps (0x532C30: the
    /// minimap's items 0x4EC8C0, its marker test 0x4CA6D0 → 0x533120, the
    /// in-game World Map 0x4E0440 and its legend 0x4EE320): not a hidden
    /// race (+0x7F, the H_ events and STAGE_0_CIRCUIT_0), in the open
    /// list (career +0x71C8, 0x5007E0), and an SUV event (+0x0D = 1) only
    /// while a career slot holds an SUV of the player's own (0x5278A0:
    /// slot +0x7F5 and car type +0x88A). The marker itself (its trigger,
    /// the Engage key, the XEE effect 0x605790) is not filtered.
    pub(crate) fn event_on_map(&self, e: &career::Event) -> bool {
        let Some(p) = self.profile.as_ref() else { return false };
        let suv = || {
            std::iter::once(&p.car)
                .chain(p.slots.iter().filter_map(|s| s.car.as_ref()))
                .any(|c| self.game_data.career.suvs.contains(c))
        };
        !e.always && p.open.contains(&e.unlock) && (!e.suv || suv())
    }

    /// The in-game map's items in the widget's order (0x4E0440): the
    /// marker table's entries (0x533320: per kind 13–15 route trigger in
    /// order, the first open shop naming it, then the first milestone on
    /// offer whose spot it is; then the open events at their markers),
    /// each at its trigger box's centre (0x4B0670); then the active AI
    /// cars (car +0x550, 0x4B0740); then the player (0x8900AC's car, its
    /// +0x60 / +0x64 and heading +0x40C).
    pub(crate) fn map_icons(&self) -> Vec<screens::MapIcon> {
        use screens::{MapIcon, map_event_kind, map_group, map_shop_kind};
        let mut icons = Vec::new();
        let Some(p) = self.profile.as_ref() else { return icons };
        let centre = |t: &career::Trigger| (t.min + t.max) * 0.5;
        if self.career_world.challenge.is_some() {
            match self.challenge_goal_marker() {
                Some((MarkerTarget::Shop(s), t)) => {
                    let kind = map_shop_kind(self.game_data.career.shops[s].kind);
                    icons.push(MapIcon {
                        group: map_group(kind),
                        kind,
                        at: centre(&t),
                        heading: None,
                        sponsor: false,
                        target: Some((self.game_data.career.shops[s].hash, t)),
                    });
                }
                Some((MarkerTarget::Shoot(h), t)) => icons.push(MapIcon {
                    group: map_group(6),
                    kind: 6,
                    at: centre(&t),
                    heading: None,
                    sponsor: false,
                    target: Some((h, t)),
                }),
                _ => {}
            }
        }
        for t in self.sim.route_triggers.iter().filter(|_| self.career_world.challenge.is_none()) {
            if let Some(s) = self.game_data.career.shops.iter().find(|s| s.trigger == t.name)
                && p.shop_open(&self.game_data.career, s)
            {
                let kind = map_shop_kind(s.kind);
                icons.push(MapIcon {
                    group: map_group(kind),
                    kind,
                    at: centre(t),
                    heading: None,
                    sponsor: false,
                    target: Some((s.hash, *t)),
                });
            }
            if let Some(m) = self.game_data.career.milestones.iter().find(|m| m.spot == t.name)
                && p.offered.contains(&m.hash)
            {
                icons.push(MapIcon {
                    group: map_group(6),
                    kind: 6,
                    at: centre(t),
                    heading: None,
                    sponsor: false,
                    target: Some((m.hash, *t)),
                });
            }
        }
        if let Some((_, starts)) = self.career_world.event_starts.as_ref().filter(|_| self.career_world.challenge.is_none()) {
            for &(i, t) in starts.iter().filter(|&&(i, _)| self.event_on_map(&self.game_data.career.events[i])) {
                let e = &self.game_data.career.events[i];
                let kind = map_event_kind(e);
                icons.push(MapIcon {
                    group: map_group(kind),
                    kind,
                    at: centre(&t),
                    heading: None,
                    sponsor: e.category == career::Category::Sponsor,
                    target: Some((e.unlock, t)),
                });
            }
        }
        for (i, t) in self.challenge_markers() {
            icons.push(MapIcon {
                group: map_group(0xF),
                kind: 0xF,
                at: centre(&t),
                heading: None,
                sponsor: false,
                target: Some((self.game_data.career.mag_triggers[i].name, t)),
            });
        }
        if let Some(e) = self.sim.encounters.as_ref() {
            icons.extend(e.cars.iter().filter(|c| c.active).map(|c| MapIcon {
                group: 0xE,
                kind: 0xE,
                at: c.state.position.truncate(),
                heading: Some(c.state.heading()),
                sponsor: false,
                target: None,
            }));
        }
        if let Some(d) = self.sim.drive.as_ref() {
            icons.push(MapIcon {
                group: 0x10,
                kind: 0x10,
                at: d.state.position.truncate(),
                heading: Some(d.state.heading()),
                sponsor: false,
                target: None,
            });
        }
        icons
    }

    /// Free roam's map icons (the minimap 0x4CA840 and the in-game World
    /// Map 0x4E0440 both take the route-trigger table's entries, 0x4EC950
    /// / 0x4EE320): the stage's events, the open shops, the photo shoots,
    /// and in an outrun its racers.
    /// …with each marker entry's GPS target (the hash 0x529490 keeps and
    /// the route trigger), None for a car.
    pub(crate) fn world_entries(&self) -> Vec<(&'static str, glam::Vec2, Option<f32>, Option<(u32, career::Trigger)>)> {
        let icons =
            self.career_world.event_starts.as_ref().filter(|_| self.career_world.challenge.is_none()).map_or_else(Vec::new, |(_, starts)| {
                starts
                    .iter()
                    .filter(|&&(i, _)| self.event_on_map(&self.game_data.career.events[i]))
                    .map(|&(i, t)| {
                        let e = &self.game_data.career.events[i];
                        let name = match (e.kind, e.category) {
                            _ if e.suv => "SUVEVENTINDICATOR_",
                            (_, career::Category::Url) => "URLEVENTINDICATOR_",
                            (career::EventKind::Sprint, _) => "SPRINTEVENTINDICATOR_",
                            (career::EventKind::Drag, _) => "DRAGEVENTINDICATOR_",
                            (career::EventKind::Drift, _) => "DRIFTEVENTINDICATOR_",
                            (career::EventKind::StreetX, _) => "STREETEVENTINDICATOR_",
                            _ => "CIRCUITEVENTINDICATOR_",
                        };
                        (name, t.position.truncate(), None, Some((e.unlock, t)))
                    })
                    .collect()
            });
        let mut icons: Vec<_> = icons;
        match self.challenge_goal_marker() {
            Some((MarkerTarget::Shop(s), t)) => {
                icons.push((self.game_data.career.shops[s].icon(), t.position.truncate(), None, Some((self.game_data.career.shops[s].hash, t))))
            }
            Some((MarkerTarget::Shoot(h), t)) => {
                icons.push(("SHOWCASEINDICATOR_", t.position.truncate(), None, Some((h, t))))
            }
            Some(_) => {}
            None if self.career_world.challenge.is_some() => {}
            None => {
                icons.extend(self.open_shops().into_iter().map(|(s, t)| {
                    (self.game_data.career.shops[s].icon(), t.position.truncate(), None, Some((self.game_data.career.shops[s].hash, t)))
                }));
                icons.extend(
                    self.shoots()
                        .into_iter()
                        .map(|(h, t)| ("SHOWCASEINDICATOR_", t.position.truncate(), None, Some((h, t)))),
                );
            }
        }
        // The world challenges on offer (type 15, legend group 0xD: the
        // showcase icon, 0x4965F0).
        icons.extend(self.challenge_markers().into_iter().map(|(i, t)| {
            ("SHOWCASEINDICATOR_", t.position.truncate(), None, Some((self.game_data.career.mag_triggers[i].name, t)))
        }));
        // An outrun (race state 7): 0x607190 makes every AI car of
        // the world list a racer (0x5FC2B0), so each gets a type-3
        // item; the ones done at the start (+0x998, all but the
        // challenged car) and parked cars stay hidden (0x4CA6D0).
        // In free roam too: the minimap's items are the car manager's AI
        // cars (0x890080 +0x14..+0x18), shown while active (car +0x550,
        // set by 0x5F6D60 as a car enters the world, cleared by 0x5F6E30)
        // and not done; in a career not while a world challenge runs
        // (0x4CA72E: 0x501690, career +0xC08 = 0x85B948, the record
        // 0x533BF0 starts).
        if let Some(e) = self
            .sim.encounters
            .as_ref()
            .filter(|_| self.career_world.challenge.is_none() || self.profile.is_none() || self.race_flow.quick_free_run)
        {
            icons.extend(
                e.cars
                    .iter()
                    .filter(|c| c.active && c.done.is_none())
                    .map(|c| ("AIRACERINDICATOR_", c.state.position.truncate(), Some(c.state.heading()), None)),
            );
        }
        icons
    }

    /// The minimap's marker test (0x4CA6D0, types 5–18, from 0x4CA73C) in
    /// a career's free roam: while the D-Day script runs only car lots
    /// from its state 7 (0x4CA74D); with the GPS on (0x4AE930 false) all
    /// in stage 0 (0x862838), else only the one at the GPS point
    /// (0x529420); with it off as before (0x501690, the legend's hide
    /// flags 0x8635A9 and 0x533120, not ported: all show).
    pub(crate) fn minimap_shows(&self, name: &str, target: Option<&(u32, career::Trigger)>) -> bool {
        let Some((_, t)) = target else { return true };
        if self.career_world.dday.running() {
            return self.career_world.dday.shows_marker(name == "CARLOTINDICATOR_");
        }
        match self.career_world.gps.as_ref().filter(|g| g.on()) {
            Some(g) => self.profile.as_ref().is_some_and(|p| p.stage == 0) || g.aims_at(t.gps),
            None => true,
        }
    }

    /// 0x532D90: with `event` the current intro event (career +0x8738),
    /// the GPS to its first goal's shop (0x4FFE20 by the goal's +0x3C) at
    /// that shop's route trigger (0x4FFC50 by its +0x3C), target the
    /// event's +8; for DDAY_EVENT_B always, else unless the GPS already
    /// aims there (0x529420).
    pub(crate) fn gps_to_intro(&mut self, event: u32) {
        if self.profile.as_ref().and_then(|p| p.intro) != Some(event) {
            return;
        }
        let Some(e) = self.game_data.career.events.iter().find(|e| e.unlock == event) else { return };
        let Some(&(_, goal)) = e.goals.first() else { return };
        let Some(shop) = self.game_data.career.shops.iter().find(|s| s.hash == goal) else { return };
        let Some(t) = self.sim.route_triggers.find(shop.trigger) else { return };
        if event == career::DDAY_EVENT_B || !self.career_world.gps.as_ref().is_some_and(|g| g.aims_at(t.gps)) {
            self.career_world.gps = Some(dday::Gps::engage(e.unlock, &t, self.car_pose()));
        }
    }

    /// MARKER_GPS_AID's point (0x5F3CC0): in free roam (no race state, so
    /// not in an outrun) with the chase camera (no scene playing) and the
    /// player's follower in state 1 or 2, the kept look-ahead point.
    pub(crate) fn gps_aid_target(&self) -> Option<Vec3> {
        if !matches!(self.mode, Mode::Explore)
            || self.scenes.shop.is_some()
            || self.sim.encounters.as_ref().is_some_and(|e| e.outrun.is_some())
        {
            return None;
        }
        self.career_world.gps.as_ref().filter(|g| matches!(g.follower.state, 1 | 2)).and_then(|g| g.aid)
    }

    /// The in-game HUD: HUD_SingleRace as the game drives it.
    /// The minimap's frame and markers: the race's track and its start
    /// line, or free roam's map (track 4000) with the stage's events.
    pub(crate) fn map_view(&mut self) -> Option<hud::MapView> {
        let (number, icons, range) = match &self.mode {
            Mode::Race(r) => {
                let track = r.test.map_or_else(|| self.game_data.career.events[r.event].track, |t| t.0);
                // 0x4EC950: an AIRACERINDICATOR_ item (type 3) per AI
                // racer (0x890080 +0x14 … +0x18); 0x4CA6D0 shows one whose
                // car is out (+0x550) and not done (+0x998, set when it
                // ends its race). The start/finish items (types 1, 2) never
                // show (0x4CA7E4).
                let icons = r
                    .racers.rivals
                    .iter()
                    .filter(|v| v.progress.finished.is_none())
                    .map(|v| ("AIRACERINDICATOR_", v.state.position.truncate(), Some(v.state.heading())))
                    .collect();
                (track, icons, 1.0)
            }
            Mode::Explore => {
                let icons = self
                    .world_entries()
                    .into_iter()
                    .filter(|e| self.minimap_shows(e.0, e.3.as_ref()))
                    .map(|(n, p, h, _)| (n, p, h))
                    .collect();
                // 0x4CA840 (0x4CA9F6..0x4CAA4A): markers at or past 1 window
                // go, past 10 instead in free roam (race state 0x89E7B0 = 0,
                // so not in an outrun) while the GPS is on (0x501100 states
                // 0–3), in stage 0 (0x862838) or while a world challenge
                // runs (0x501690). Without a profile the stage is taken as 0
                // prov: inferred game.map_view
                // (**inferred**: nothing loaded a career's stage).
                let outrun = self.sim.encounters.as_ref().is_some_and(|e| e.outrun.is_some());
                let wide = self.career_world.gps.as_ref().is_some_and(|g| g.on())
                    || self.profile.as_ref().is_none_or(|p| p.stage == 0)
                    || self.career_world.challenge.is_some();
                let range = if !outrun && wide { 10.0 } else { 1.0 };
                (4000, icons, range)
            }
            Mode::Menu(_) => return None,
        };
        let texture = self.game_data.assets.track_map(&self.game_data.root, number);
        let t = self.game_data.career.tracks.get(&number)?;
        let d = self.sim.drive.as_ref()?;
        let gps = self
            .career_world.gps
            .as_ref()
            .filter(|_| matches!(self.mode, Mode::Explore) && self.profile.is_some() && !self.race_flow.quick_free_run);
        Some(hud::MapView {
            texture,
            left: t.map_left,
            top: t.map_top,
            size: t.map_size,
            zoom: t.map_zoom,
            player: d.state.position.truncate(),
            heading: d.state.heading(),
            // 0x4CA840: in a race state (0x89E7B0 ≠ 0, outruns too) the
            // race map mode 0x83AA1B, else the explore one 0x83AA1C; 2
            // turns the map with the car.
            rotate: if matches!(self.mode, Mode::Race(_))
                || self.sim.encounters.as_ref().is_some_and(|e| e.outrun.is_some())
            {
                self.settings.race_map == 2
            } else {
                self.settings.explore_map == 2
            },
            icons,
            range,
            // 0x4DA640 → 0x4C8410: an engaged GPS (+0x29) in a career's
            // free roam puts 0x697D2BCA at its map point (0x5010C0 →
            // 0x4CB150 → 0x4CAE60).
            gps: gps.map(|g| g.map),
            // 0x4C8410: GPS_Search_Group while the search runs or failed
            // and the follower holds no route; GPS_SEARCH_ERROR with the
            // state's text (0x155409E8 / 0x31139B65).
            gps_searching: gps.is_some_and(|g| g.searching()),
            gps_error: gps.and_then(|g| g.error()).map(|h| {
                self.label(h, if h == 0x1554_09E8 { "Unable to contact satellite" } else { "Unable to find a path" })
            }),
        })
    }
}
