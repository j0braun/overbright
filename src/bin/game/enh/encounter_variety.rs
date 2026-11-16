use crate::*;

/// The ticks after which the deterministic headless modes take an Outrun
/// Variety load (a car loads in well under this on the reference machine).
const VARIETY_WAIT_TICKS: u64 = 60;

impl Game {
    /// Free roam's encounter racers' cars as its set-up (`explore`) will
    /// load them for the player's car `car`, predicted from the random
    /// numbers as they stand (the set-up's draws before them replayed on a
    /// copy: the traffic's seed when it loads again).
    /// Outrun Variety (Enhancements, clone deviation; off = native): a
    /// parked encounter racer that has been on the road gets a new car
    /// for its slot, drawn by the set-up's rule against the same player
    /// rating (0x4FEAF0's pool, 0x6371F0 paint, 0x4FED00 skill row,
    /// 0x532020 match and dress: the same pool, class and performance as
    /// the set-up's, so never stronger or weaker), on the racers' own
    /// random numbers (the game's are not touched), avoiding the car
    /// types out now. Loaded on a thread; swapped in only while that slot
    /// is still parked and no outrun runs.
    pub(crate) fn outrun_variety_frame(&mut self) {
        if nfsu2::qol::outrun_variety() {
            self.outrun_variety_step();
        }
    }

    /// [`Game::outrun_variety_frame`] with the row on.
    pub(crate) fn outrun_variety_step(&mut self) {
        let human = match self.sim.ai.as_ref() {
            Some(ai) if self.sim.encounters.as_ref().is_some_and(|e| e.variety.iter().any(|&v| v)) => self.human_perf(ai),
            _ => None,
        };
        let (Some(e), Some(d), Some(ai)) = (self.sim.encounters.as_mut(), self.sim.drive.as_ref(), self.sim.ai.as_ref()) else {
            return;
        };
        if e.pending.is_some() || e.cars.is_empty() {
            return;
        }
        for (seen, c) in e.variety.iter_mut().zip(&e.cars) {
            *seen |= c.active;
        }
        // The load is taken when it is done; in the deterministic headless
        // modes (`surface_maps::blocking`: shots, benches, flows) at exactly
        // VARIETY_WAIT_TICKS ticks after it was asked for, waiting for it
        // then, so the swap's tick never depends on the loading thread's
        // timing.
        let tick = self.pacing.clocks.tick;
        let due = |l: &(usize, usize, u32, std::thread::JoinHandle<Result<Car>>, u64)| {
            if nfsu2::surface_maps::blocking() { tick >= l.4 + VARIETY_WAIT_TICKS } else { l.3.is_finished() }
        };
        if e.variety_load.as_ref().is_some_and(due) {
            let (slot, row, seed, job, _) = e.variety_load.take().expect("the variety load");
            match job.join() {
                Ok(Ok(car)) if e.outrun.is_none() && e.cars.get(slot).is_some_and(|c| !c.active) => {
                    let old = &e.cars[slot];
                    let mut new = encounter_car(car, row, seed, e.throttle);
                    (new.placed, new.held, new.done) = (old.placed, old.held, old.done);
                    e.cars[slot] = new;
                    e.variety[slot] = false;
                }
                Ok(Err(err)) => eprintln!("outrun variety: {err:#}"),
                _ => {}
            }
            return;
        }
        if e.variety_load.is_some() || e.outrun.is_some() {
            return;
        }
        let Some(slot) = (0..e.cars.len()).find(|&i| e.variety[i] && !e.cars[i].active) else { return };
        let types: Vec<u8> = e
            .cars
            .iter()
            .filter_map(|c| {
                self.game_data.car_types.iter().position(|t| t.name.eq_ignore_ascii_case(&c.car.car_type.name)).map(|k| k as u8)
            })
            .collect();
        let draw = EncounterDraw::new(&self.game_data.car_types, ai, &d.car);
        let difficulty = self.sim.race_globals.difficulty;
        let Some(mut pick) = draw.pick(&self.game_data.car_types, &self.game_data.parts, ai, difficulty, &mut e.rng, &types) else {
            return;
        };
        let fitted = EncounterDraw::dress(
            &self.game_data.car_types,
            &self.game_data.parts,
            (&self.game_data.career.packages, &self.game_data.car_records, human),
            ai,
            &mut pick,
            &mut e.rng,
        );
        let (root, packages, name, installed) = (
            self.game_data.root.clone(),
            self.game_data.career.packages.clone(),
            self.game_data.car_types[pick.0 as usize].name.clone(),
            std::mem::take(&mut pick.1),
        );
        let job = std::thread::Builder::new()
            .name("outrun variety".into())
            .spawn(move || Car::load_upgraded(&root, &name, &installed, &packages, &|id| fitted.contains(&id)));
        match job {
            Ok(job) => e.variety_load = Some((slot, pick.2, pick.3, job, tick)),
            Err(err) => eprintln!("outrun variety: {err}"),
        }
    }
}
