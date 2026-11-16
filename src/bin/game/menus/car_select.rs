use crate::*;

impl Game {
    /// The player's car: its point (+0x60) and forward row (+0x90) in x, y.
    pub(crate) fn car_pose(&self) -> Option<(Vec3, Vec2)> {
        self.sim.drive.as_ref().map(|d| {
            let f = d.state.forward();
            (d.state.position, Vec2::new(f.x, f.y))
        })
    }

    pub(crate) fn label(&self, hash: u32, fallback: &str) -> String {
        self.game_data.assets.strings.get(hash).unwrap_or(fallback).to_string()
    }

    /// UI_CareerCarLot (0x4F9930): the stock cars on offer (list mode 1),
    /// the first one selected, in the state the player's slots call for
    /// (0x495F00).
    pub(crate) fn career_lot(&self) -> Mode {
        let Some(p) = self.profile.as_ref() else { return self.icon_menu(MenuKind::Career) };
        let cars = p
            .lot_cars(&self.game_data.career, &self.game_data.car_types)
            .into_iter()
            .map(|i| CarChoice::new(&self.game_data.assets, &self.game_data.car_types[i].name, None))
            .collect();
        let state = if p.free_slot().is_some() {
            LotState::Take
        } else if p.car_slots().len() == 1 {
            LotState::TradeIn
        } else {
            LotState::TradeOrBrowse
        };
        Mode::Menu(Menu::CarLot(CarLot {
            title: String::new(),
            cars,
            selected: 0,
            opened: self.now(),
            state,
            stock: None,
            filter: 3,
            arrows: Default::default(),
        }))
    }

    /// UI_QRTrackSelect for a mode on its record's track (0x4CDED0), the
    /// tracks' maps loaded (the widget loads TRACKMAP<n> as it shows it).
    pub(crate) fn quick_track_select(&mut self, mode: screens::QrMode, now: f32) -> Option<QuickRace> {
        let ty = nfsu2::frontend::quick_race::race_type(mode);
        let (data, profile) = (&self.game_data.career, self.profile.as_ref());
        let won = |e: &career::Event| profile.is_some_and(|p| p.won(data, e.unlock));
        let qr = QuickRace::for_mode(&self.game_data.career, mode, &mut self.settings.qr[ty], &won, now)?;
        for t in &qr.tracks {
            self.game_data.assets.track_map(&self.game_data.root, t.number);
        }
        Some(qr)
    }

    /// Whether a track is open in Quick Race (0x5156F0).
    pub(crate) fn quick_track_open(&self, track: u16) -> bool {
        let profile = self.profile.as_ref();
        nfsu2::frontend::quick_race::unlocked(&self.game_data.career, track, &|e| {
            profile.is_some_and(|p| p.won(&self.game_data.career, e.unlock))
        })
    }

    /// UI_QRModeOptions for the mode's record.
    pub(crate) fn quick_options_screen(&self, mode: screens::QrMode, now: f32) -> Mode {
        let ty = nfsu2::frontend::quick_race::race_type(mode);
        let r = &self.settings.qr[ty];
        let rows =
            nfsu2::frontend::quick_race::rows(ty, self.game_data.career.tracks.get(&r.track), self.quick_track_open(r.track), r);
        Mode::Menu(Menu::QrOptions(nfsu2::frontend::quick_race::QrOptions::new(mode, &rows, now)))
    }

    /// The race the mode's record sets up (0x53F160 / 0x53EEA0), as an
    /// event at the end of the event table (the one slot Quick Race
    /// reuses): its opponents' names (0x4FE970) and skill rows (0x4FED00
    /// at the record's difficulty) drawn now; drift tracks with +0x80 = 1
    /// race alone (0x4CD7A0: 0x89E7C4 = 0).
    pub(crate) fn quick_event(&mut self, mode: screens::QrMode) -> usize {
        let ty = nfsu2::frontend::quick_race::race_type(mode);
        let r = self.settings.qr[ty];
        let alone = ty == 3 && self.game_data.career.tracks.get(&r.track).is_some_and(|t| t.drift_kind == 1);
        let n = if alone { 0 } else { r.opponents as usize };
        let names = nfsu2::frontend::quick_race::draw_names(&mut self.rng, n);
        let mut rows: Vec<u8> = (0..n)
            .map(|_| {
                self.sim.ai.as_ref().map_or(0, |a| a.tuning.random_row(r.difficulty as usize, self.rng.unit(1.0))) as u8
            })
            .collect();
        // Bench aid (not native): NFSU2_BENCH_RIVAL_ROWS=6,7,8 forces the
        // rivals' skill rows, as writing racer +0x901 does in the original.
        if let Some(v) = debug_env().bench_rival_rows.clone() {
            for (row, f) in rows.iter_mut().zip(v.split(',')) {
                if let Ok(x) = f.trim().parse() {
                    *row = x;
                }
            }
        }
        let index = match self.quick.event {
            Some(i) => i,
            None => {
                self.quick.event = Some(self.game_data.career.events.len());
                self.game_data.career.events.len()
            }
        };
        let e = nfsu2::frontend::quick_race::event(mode, &r, index, false, names, &rows);
        if index < self.game_data.career.events.len() {
            self.game_data.career.events[index] = e;
        } else {
            self.game_data.career.events.push(e);
        }
        self.quick.mode = Some(mode);
        index
    }

    /// The cars UI_QRCarSelect lists for a filter (garage records whose
    /// +0x14 has a bit of it, 0x4E22E0): the stock entries (bit 1), then
    // prov: inferred game.qr_cars
    /// the My Cars records (bit 2) — garage order (**inferred**: the list's
    /// walk is not traced).
    pub(crate) fn qr_cars(&self, filter: u32) -> Vec<CarChoice> {
        let fresh = Profile::new("Player");
        let p = self.profile.as_ref().unwrap_or(&fresh);
        let mut cars: Vec<CarChoice> = Vec::new();
        if filter & 1 != 0 {
            cars.extend(
                p.quick_race_cars(&self.game_data.career, &self.game_data.car_types)
                    .into_iter()
                    .map(|i| CarChoice::new(&self.game_data.assets, &self.game_data.car_types[i].name, None)),
            );
        }
        if filter & 2 != 0 {
            for (k, m) in p.my_cars.iter().enumerate() {
                if let Some(m) = m.as_ref().filter(|m| !m.car.is_empty()) {
                    let mut c = CarChoice::new(&self.game_data.assets, &m.car, None);
                    c.my_car = Some(k);
                    cars.push(c);
                }
            }
        }
        if filter & 8 != 0 {
            cars.extend(self.sponsor_cars());
        }
        cars
    }

    /// The "SPONSOR CARS" records (bit 8): the eight the garage holds
    /// (0x5163C0, records 73–80, id "SPONSOR_<preset>", the preset's name
    /// hash at +0x18) that Quick Race opens — 0x529F80 finds the id in the
    /// garage's table (0x84481C) and asks 0x579D80, the code typed at the
    /// title (`cheats`). Under Wine with all eight codes: Navigator, H2,
    /// Eclipse, IS 300, Corolla, G35, Escalade, GTO.
    pub(crate) fn sponsor_cars(&self) -> Vec<CarChoice> {
        nfsu2::frontend::cheats::SPONSOR_CARS
            .iter()
            .filter(|&&name| nfsu2::frontend::cheats::sponsor_car(name))
            .filter_map(|&name| {
                let preset = self.game_data.presets.iter().find(|x| x.name == name)?;
                let mut c = CarChoice::new(&self.game_data.assets, &preset.car, None);
                c.preset = Some(name.to_string());
                Some(c)
            })
            .collect()
    }

    /// Whether a filter has cars (`next_qr_filter` skips empty ones).
    pub(crate) fn qr_filter_has(&self, filter: u32) -> bool {
        match filter {
            2 => self.profile.as_ref().is_some_and(|p| p.my_cars.iter().flatten().any(|m| !m.car.is_empty())),
            8 => !self.sponsor_cars().is_empty(),
            _ => false,
        }
    }

    /// A sponsor car's preset parts (`Preset::installed`, `Preset::perf`).
    pub(crate) fn preset_ride(
        &self,
        name: &str,
    ) -> Option<(std::collections::BTreeMap<u8, u32>, std::collections::BTreeSet<u8>)> {
        let x = self.game_data.presets.iter().find(|x| x.name == name)?;
        Some((x.installed(), x.perf(&self.game_data.career.packages)))
    }

    /// UI_QRCarSelect in Quick Race (game mode 2; class 0x4F9D90, list
    /// 0x4E22E0 with the profile's filter 0x83AAC0, "All Cars" 3 when the
    /// filter has none, 0x534860): the filter "Stock Cars" (1), the
    /// profile's stock car records (0x5162D0) unlocked for Quick Race
    /// (0x534190). The filter starts at All Cars (0x7F444C = 3) and the
    /// triggers switch it (`next_qr_filter`). The Quick Race unlocks are
    /// the career lot's plus 0x529CD0's eight (`Profile::quick_race_cars`,
    /// **verified** against a fresh profile under Wine, menus15); Sponsor 8
    /// holds the sponsor cars whose codes were typed at the title
    /// (`Game::sponsor_cars`); Career 4 and Ranked 0x10 have no records here.
    /// `event` None is Free Run.
    pub(crate) fn quick_car_select(&self, event: Option<usize>) -> Mode {
        let cars = self.qr_cars(self.quick.filter);
        let selected = self.quick.car.as_ref().and_then(|c| cars.iter().position(|x| &x.name == c)).unwrap_or(0);
        let mode = match event {
            Some(e) => self.game_data.career.events[e].kind.label().to_string(),
            None => self.label(0x6F24_21B7, "Free Run"),
        };
        Mode::Menu(Menu::CarLot(CarLot {
            title: mode,
            cars,
            selected,
            opened: self.now(),
            state: LotState::QuickRace(event),
            stock: None,
            filter: self.quick.filter,
            arrows: Default::default(),
        }))
    }

    /// The player's cars in their slots: the crib's Car Select
    /// (UICareerCribCarSelect, 0x4FC110 → 0x4F9AD0; 0x4E0160 lists every
    /// slot, locked and empty ones as placeholders), or the car lot's state 5 (0x4DF9C0 in list mode 4)
    /// trading one of them for `stock`.
    pub(crate) fn own_cars(&self, stock: Option<String>) -> Mode {
        let Some(p) = self.profile.as_ref() else { return self.icon_menu(MenuKind::Career) };
        let cars: Vec<CarChoice> = if stock.is_some() {
            p.car_slots().into_iter().map(|(i, c)| CarChoice::new(&self.game_data.assets, &c, Some(i))).collect()
        } else {
            p.all_slots()
                .into_iter()
                .map(|(i, unlocked, car)| match (unlocked, car) {
                    (false, _) => CarChoice::placeholder(i, Placeholder::Locked),
                    (true, None) => CarChoice::placeholder(i, Placeholder::Open),
                    (true, Some(c)) => CarChoice::new(&self.game_data.assets, &c, Some(i)),
                })
                .collect()
        };
        // The crib's select opens on the list's head, slot 1 (0x4E0160 puts
        // +0x54 there and the constructor 0x4F9AD0 does not move it).
        let selected = if stock.is_some() { cars.iter().position(|c| c.slot == Some(p.slot)).unwrap_or(0) } else { 0 };
        let state = if stock.is_some() { LotState::Browse } else { LotState::CribSelect };
        Mode::Menu(Menu::CarLot(CarLot {
            title: String::new(),
            cars,
            selected,
            opened: self.now(),
            state,
            stock,
            filter: 3,
            arrows: Default::default(),
        }))
    }

    /// A question with the car names put in its `%s` (the lot's dialogs,
    /// 0x5587A0 with CAR_NAME_<car>, 0x5120B0), and its two buttons.
    pub(crate) fn ask_cars(
        &self,
        question: u32,
        cars: &[&str],
        buttons: [(u32, &str); 2],
        outcomes: [Mode; 2],
    ) -> Mode {
        let mut text = self.label(question, "?");
        for car in cars {
            text = text.replacen("%s", &self.game_data.assets.strings.label(&format!("CAR_NAME_{car}")), 1);
        }
        // No package named (0x4DFEC6, 0x4EDC86): 0x526C50 sizes it.
        let package = screens::dialog_package(&self.game_data.assets, &text);
        let labels = buttons.map(|(h, fallback)| self.label(h, fallback));
        Mode::Menu(Menu::Dialog(Dialog::new(package, &text, &[&labels[0], &labels[1]], self.now()), outcomes.into()))
    }
}

/// The Quick Race set-up and its car select (were `Game` fields).
pub(crate) struct QuickRaceState {
    /// The car UI_QRCarSelect picked for a Quick Race or Free Run, and the
    /// track list it came from.
    pub(crate) car: Option<String>,
    /// UI_QRCarSelect's filter (0x7F444C, starts at 3, All Cars).
    pub(crate) filter: u32,
    pub(crate) tracks: Option<QuickRace>,
    /// UI_QRModeOptions as left for the car select (its Back returns there).
    pub(crate) options: Option<nfsu2::frontend::quick_race::QrOptions>,
    /// The event slot a Quick Race races on (see `quick_event`) and the
    /// mode it was set up for.
    pub(crate) event: Option<usize>,
    pub(crate) mode: Option<screens::QrMode>,
    /// The Quick Race car select a customize session (0x552CD0(2, …))
    /// goes back to.
    pub(crate) customize: Option<CarLot>,
    /// The chosen Quick Race car's parts and performance parts when it is
    /// a My Cars record.
    pub(crate) parts: Option<(std::collections::BTreeMap<u8, u32>, std::collections::BTreeSet<u8>)>,
}

impl Default for QuickRaceState {
    fn default() -> QuickRaceState {
        QuickRaceState {
            car: None,
            customize: None,
            parts: None,
            filter: 3,
            tracks: None,
            options: None,
            event: None,
            mode: None,
        }
    }
}
