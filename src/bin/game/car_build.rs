use crate::*;

impl Game {
    /// `perf` with the career car's performance uniques added as
    /// `performance::UNIQUE_BASE + i` (the upgrade state's flags +0x28 + 4i,
    /// which 0x5997F0 applies after the parts), for the physics only.
    pub(crate) fn with_uniques(&self, perf: &std::collections::BTreeSet<u8>) -> std::collections::BTreeSet<u8> {
        let mut all = perf.clone();
        if self.quick.car.is_none()
            && let Some(p) = self.profile.as_ref()
        {
            all.extend(p.perf_uniques.iter().filter(|&&i| i < 10).map(|&i| nfsu2::performance::UNIQUE_BASE + i));
        }
        all
    }

    /// The career car's record with these performance parts (0x5AC500).
    pub(crate) fn perf_record(&self, fitted: &std::collections::BTreeSet<u8>) -> Option<Vec<u8>> {
        let name = self.profile.as_ref().map_or("PEUGOT", |p| if p.car.is_empty() { "PEUGOT" } else { p.car.as_str() });
        let (_, record) = self.game_data.car_records.iter().find(|(n, _)| n == name)?;
        let mut record = record.clone();
        let mut fitted = self.with_uniques(fitted);
        if let Some(p) = self.profile.as_ref().filter(|p| !p.car.is_empty()) {
            self.fit_aero(&p.car, &p.installed, &mut fitted);
        }
        nfsu2::performance::upgrade(&mut record, &self.game_data.career.packages, &|id| fitted.contains(&id));
        Some(record)
    }

    /// The player's racer as 0x5B7580 reads it (`ai::route::HumanPerf`)
    /// when 0x532020 runs: the career ride's state as stored — Rachel's
    /// D-Day preset (no car bought yet) without the aerodynamics its body
    /// parts give, which only its racer's creation (0x639CF0(0)) adds; a
    /// bought car with them (the shops refresh it through 0x639CF0,
    // prov: inferred game.human_perf
    /// **inferred** to persist) — with the uniques: its rating (+8, the
    /// free-roam set-up's area) and handling score (0x5B54A0) from that
    /// record, and its nitrous level (upgrade state float 2 + 0.5,
    /// truncated). Checked under Wine: the three encounter racers of a new
    /// career match only without the D-Day car's body aero (score 0.6085
    // prov: open game.human_perf-2
    /// against 0.6585 with it). The dyno tuning is left out (**open**).
    pub(crate) fn human_perf(&self, ai: &AiData) -> Option<nfsu2::ai::route::HumanPerf> {
        let (car, perf) = match self.profile.as_ref() {
            _ if self.quick.car.is_some() => (self.quick.car.clone()?, std::collections::BTreeSet::new()),
            Some(p) if !p.car.is_empty() => {
                let mut perf = p.perf.clone();
                self.fit_aero(&p.car, &p.installed, &mut perf);
                (p.car.clone(), perf)
            }
            Some(_) => {
                let x = self.game_data.presets.iter().find(|x| x.name == career::DDAY_PLAYER_CAR)?;
                (x.car.clone(), x.perf(&self.game_data.career.packages))
            }
            None => (self.sim.drive.as_ref()?.car.car_type.name.clone(), std::collections::BTreeSet::new()),
        };
        let all = self.with_uniques(&perf);
        let (_, record) = self.game_data.car_records.iter().find(|(n, _)| n.eq_ignore_ascii_case(&car))?;
        let mut record = record.clone();
        nfsu2::performance::upgrade(&mut record, &self.game_data.career.packages, &|id| all.contains(&id));
        let mut h = nfsu2::handling::parse_handling(&record);
        nfsu2::handling::setup_performance(&mut h, nfsu2::race::RaceMode::FreeRoam);
        Some(nfsu2::ai::route::HumanPerf {
            rating: h.performance_rating,
            score: nfsu2::ai::route::handling_score(&record, &ai.bounds),
            nitrous: (nfsu2::performance::state_float(&self.game_data.career.packages, &|id| perf.contains(&id), 2) + 0.5) as u8,
        })
    }

    /// The career car's stock and upgraded car-type records.
    pub(crate) fn tuning_records(&self) -> Option<(Vec<u8>, Vec<u8>)> {
        let p = self.profile.as_ref()?;
        let g = nfsu2::car::shared_file(&nfsu2::game_fs::path(&self.game_data.root, "GLOBAL/GLOBALB.BUN")).ok()?;
        let stock = nfsu2::car::car_type_records(&g).ok()?.into_iter().find(|(n, _)| n.eq_ignore_ascii_case(&p.car))?.1;
        let mut up = stock.clone();
        let perf = self.with_uniques(&p.perf);
        nfsu2::performance::upgrade(&mut up, &self.game_data.career.packages, &|id| perf.contains(&id));
        Some((stock, up))
    }

    /// The career car's forward gears with its performance parts (tuning
    /// block +0x1C8 after 0x5AC500).
    /// Kept for the car and parts it was worked out for: the tuning
    /// screens ask for it every frame, and the handling's parse each time
    /// gave the same number.
    pub(crate) fn tuned_gears(&self) -> u32 {
        let Some(p) = self.profile.as_ref() else { return 0 };
        let key = (p.car.clone(), self.with_uniques(&p.perf));
        let mut memo = self.dyno.gears.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some((k, gears)) = memo.as_ref()
            && *k == key
        {
            return *gears;
        }
        // A failed read is not kept (the next call reads again).
        let Some(h) = self.tuned_handling() else { return 0 };
        let gears = h.gearbox.ratios.len().saturating_sub(2) as u32;
        *memo = Some((key, gears));
        gears
    }

    /// The career car's handling with its performance parts.
    pub(crate) fn tuned_handling(&self) -> Option<nfsu2::handling::Handling> {
        let p = self.profile.as_ref()?;
        let perf = self.with_uniques(&p.perf);
        let fitted = |id: u8| perf.contains(&id);
        let g = nfsu2::car::shared_file(&nfsu2::game_fs::path(&self.game_data.root, "GLOBAL/GLOBALB.BUN")).ok()?;
        nfsu2::car::parse_car_type_upgraded(&g, &p.car, &self.game_data.career.packages, &fitted).ok().map(|t| t.handling)
    }

    /// The career car's handling with its performance parts and the dyno's
    /// sliders (the dyno race's racer, 0x526980(1): built with the mode's
    /// settings and the sliders 0x83A611 copied over; the tuning object's
    /// block, which the screens' writer 0x5B3FB0 tunes in place).
    pub(crate) fn dyno_handling(&self) -> Option<nfsu2::handling::Handling> {
        let p = self.profile.as_ref()?;
        let perf = self.with_uniques(&p.perf);
        let fitted = |id: u8| perf.contains(&id);
        let settings = self.dyno_settings();
        let g = nfsu2::car::shared_file(&nfsu2::game_fs::path(&self.game_data.root, "GLOBAL/GLOBALB.BUN")).ok()?;
        nfsu2::car::parse_car_type_tuned(&g, &p.car, &self.game_data.career.packages, &fitted, Some((&self.game_data.career.tuning, &settings)))
            .ok()
            .map(|t| t.handling)
    }

    /// The car the player drives: the current slot's, or until the car
    /// lot gives one, Rachel's car — the D-Day preset (0x637280).
    pub(crate) fn player_car(&self) -> (String, std::collections::BTreeMap<u8, u32>, std::collections::BTreeSet<u8>) {
        if let Some(car) = &self.quick.car {
            if let Some((installed, perf)) = &self.quick.parts {
                let mut perf = perf.clone();
                self.fit_aero(car, installed, &mut perf);
                return (car.clone(), installed.clone(), perf);
            }
            return (car.clone(), Default::default(), Default::default());
        }
        let (car, installed, mut perf) = match self.profile.as_ref() {
            Some(p) if !p.car.is_empty() => (p.car.clone(), p.installed.clone(), p.perf.clone()),
            Some(_) => match self.game_data.presets.iter().find(|x| x.name == career::DDAY_PLAYER_CAR) {
                Some(x) => (x.car.clone(), x.installed(), x.perf(&self.game_data.career.packages)),
                None => return ("240SX".into(), Default::default(), Default::default()),
            },
            None => return ("240SX".into(), Default::default(), Default::default()),
        };
        self.fit_aero(&car, &installed, &mut perf);
        // The career ride built from the record (0x503880: 0x631D30 per
        // slot, so 0x61BCE0's derived slots even over stale ones).
        let installed = nfsu2::car::with_derived_slots(&installed, &self.game_data.parts, &car);
        (car, installed, perf)
    }

    /// The aerodynamics the ride's visual parts give it (0x6399E0 by
    /// 0x639CF0, `performance::fit_aero`).
    pub(crate) fn fit_aero(
        &self,
        car: &str,
        installed: &std::collections::BTreeMap<u8, u32>,
        perf: &mut std::collections::BTreeSet<u8>,
    ) {
        let level = |slot: u8| {
            installed.get(&slot).and_then(|&h| self.game_data.parts.find(car, slot as usize, Some(h), None)).map(|p| p.level)
        };
        nfsu2::performance::fit_aero(perf, &self.game_data.career.packages, &level);
    }
}
