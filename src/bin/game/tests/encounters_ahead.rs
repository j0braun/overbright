//! Free roam's encounter racers loaded ahead in the menus
//! (`EncountersAhead`, docs/first-use-builds.md "Encounter racers
//! ahead"): the set-up's draws predicted behind the menus are the set-up's
//! own on every way into career free roam, so it takes every car loaded
//! ahead, and free roam is the same with the loads ahead and without them.
use super::*;

const SIZE: (u32, u32) = (64, 48);
const TICK: f32 = 1.0 / 60.0;

/// One of two games walked through the same steps: `ahead` with the
/// encounter racers' loads ahead as the game makes them, the other with
/// nothing loaded ahead at its loads.
struct Walk {
    game: Game,
    ahead: bool,
}

impl Walk {
    /// A career at `stage` with `wins` of each earlier stage's plain events
    /// and of its own won, saved as the harness's profile (current, as the
    /// boot loads it), on the main menu with an offscreen window.
    fn career(stage: u8, wins: usize, ahead: bool) -> Walk {
        let mut game = Game::new().unwrap();
        let mut p = Profile::new("ENCOUNTERS_AHEAD");
        p.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
        for s in 1..=stage {
            let events: Vec<_> = game
                .game_data
                .career
                .events
                .iter()
                .filter(|e| e.stage == s && e.category == career::Category::Normal)
                .cloned()
                .collect();
            let n = if s < stage { events.len() } else { wins.min(events.len()) };
            for e in &events[..n] {
                p.win_paid(&game.game_data.career, &game.game_data.root, e, 60.0, None);
            }
        }
        p.stage = stage;
        p.save().unwrap();
        // As the boot loads it: from the file.
        game.profile = Profile::load();
        assert!(game.profile.is_some(), "the harness's profile loaded");
        game.settings.save_prompts = true;
        super::tick_effects::offscreen_window(&mut game, SIZE.0, SIZE.1);
        game.mode = game.icon_menu(MenuKind::Main);
        Walk { game, ahead }
    }

    /// `secs` of the window's steps, each tick's 1/60 s in real time as
    /// the window paces them (the loads ahead run on threads beside them).
    fn wait(&mut self, secs: f32) {
        for _ in 0..(secs / TICK).round() as usize {
            let t = Instant::now();
            self.game.step_for(TICK).unwrap();
            if let Some(rest) = std::time::Duration::from_secs_f32(TICK).checked_sub(t.elapsed()) {
                std::thread::sleep(rest);
            }
        }
    }

    /// `n` steps as fast as they go (a race driven through).
    fn ticks(&mut self, n: usize) {
        for _ in 0..n {
            self.game.step_for(TICK).unwrap();
        }
    }

    /// Before a key that may start free roam: without the loads ahead,
    /// nothing loaded ahead.
    fn before_load(&mut self) {
        if !self.ahead {
            self.game.loads.encounters_ahead = None;
        }
    }

    fn key(&mut self, key: Key) {
        self.before_load();
        self.game.menu_key(key).unwrap();
        self.wait(TICK);
    }

    fn act(&mut self, action: Action, kind: MenuKind) {
        self.before_load();
        self.game.act(action, kind).unwrap();
        self.wait(TICK);
    }

    /// A question's first answer (Yes, OK).
    fn confirm(&mut self) {
        if let Mode::Menu(Menu::Dialog(d, _)) = &self.game.mode
            && d.selected != 0
        {
            self.game.menu_key(Key::Left).unwrap();
        }
        self.key(Key::Accept);
    }

    fn menu(&self) -> String {
        crate::dev::util::flow_mode(&self.game.mode)
    }

    /// The main menu up five seconds (its loads ahead, the world, the car
    /// and the racers, are in long before on a quiet machine; the two games
    /// stay the same number of ticks either way).
    fn settle(&mut self) {
        self.wait(5.0);
    }

    /// Yes / Accept through the questions and messages on the way (the
    /// save point's, each up `secs`) until free roam runs.
    fn into_city(&mut self, secs: f32, what: &str) {
        for _ in 0..16 {
            if matches!(self.game.mode, Mode::Explore) {
                return;
            }
            self.confirm();
            self.wait(secs);
        }
        panic!("{what}: not in free roam ({})", self.menu());
    }

    /// Career → Resume Career → the garage (up `secs`) → Return to Explore
    /// Mode, its question up `secs` too, Yes.
    fn resume(&mut self, secs: f32) {
        self.act(Action::Career, MenuKind::Main);
        self.wait(secs);
        self.act(Action::ResumeCareer, MenuKind::Career);
        assert!(matches!(self.game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Crib))), "the garage: {}", self.menu());
        self.wait(secs);
        self.leave_garage(secs);
    }

    fn leave_garage(&mut self, secs: f32) {
        self.act(Action::Explore, MenuKind::Crib);
        self.wait(secs);
        self.into_city(secs, "out of the garage");
    }

    /// A city sprint of the stage raced to its end (the player moved along
    /// the road), each screen after it up `secs`, Return to Explore Mode.
    fn race_and_back(&mut self, secs: f32) {
        let g = &self.game;
        let stage = g.profile.as_ref().map_or(1, |p| p.stage);
        let event = g
            .game_data
            .career
            .stage_events(&g.game_data.root, stage)
            .iter()
            .find(|e| {
                e.kind == career::EventKind::Sprint
                    && g.game_data.career.tracks.get(&e.races[0].0).is_some_and(|t| t.region == EXPLORE_REGION)
            })
            .map(|e| e.index)
            .expect("a city sprint");
        self.game.start_race(event, false, false).unwrap();
        for _ in 0..20_000 {
            let Mode::Race(race) = &self.game.mode else { break };
            if race.timing.clock > 0.5 && race.racers.me.finished.is_none() {
                let target = race.route.road.point_at(race.racers.me.along + 60.0);
                let d = self.game.sim.drive.as_mut().unwrap();
                d.state.position = target.extend(d.state.position.z);
                self.ticks(10);
            } else {
                self.ticks(1);
            }
        }
        // The results, the reward, a sponsor's offer (Yes) on the way.
        for _ in 0..12 {
            if matches!(self.game.mode, Mode::Menu(Menu::Icons(_, MenuKind::PostRace))) {
                break;
            }
            self.wait(secs);
            self.confirm();
        }
        assert!(matches!(self.game.mode, Mode::Menu(Menu::Icons(_, MenuKind::PostRace))), "post race: {}", self.menu());
        self.wait(secs);
        self.act(Action::ReturnToExplore, MenuKind::PostRace);
        self.wait(secs);
        self.into_city(secs, "after the race");
    }

    /// A body shop's screens (as its drive-in scene opens them), up
    /// `secs`, then its exit question's Yes.
    fn shop_and_back(&mut self, secs: f32) {
        let i = self.game.game_data.career.shops.iter().position(|s| s.kind == 2).expect("a body shop");
        self.game.shop.visit = Some(self.game.game_data.career.shops[i].hash);
        self.game.mode = self.game.shop_front_end(i);
        self.wait(secs);
        self.before_load();
        self.game.leave_shop().unwrap();
        self.wait(TICK);
        self.into_city(secs, "out of the shop");
    }

    /// Free roam's pause, Jump to Garage, the garage up `secs`, back.
    fn jump_to_garage(&mut self, secs: f32) {
        self.game.key(KeyCode::Escape, &mut || {}).unwrap();
        self.wait(secs);
        assert!(
            matches!(self.game.mode, Mode::Menu(Menu::Icons(_, MenuKind::ExplorePause))),
            "explore's pause: {}",
            self.menu()
        );
        self.act(Action::JumpToGarage, MenuKind::ExplorePause);
        self.wait(secs);
        // "Jump to Garage?": OK.
        self.confirm();
        assert!(matches!(self.game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Crib))), "the garage: {}", self.menu());
        self.wait(secs);
        self.leave_garage(secs);
    }

    /// From free roam to the main menu (the pause's Quit), a Quick Race
    /// circuit, its pause's Quit, then Resume Career (the drive of the Quick
    /// Race still in memory on the career's menus).
    fn quick_race_then_resume(&mut self, secs: f32) {
        self.game.key(KeyCode::Escape, &mut || {}).unwrap();
        self.wait(secs);
        self.act(Action::MainMenu, MenuKind::ExplorePause);
        self.into_menu(secs);
        self.game.quick.tracks = self.game.quick_track_select(screens::QrMode::Circuit, 0.0);
        let e = self.game.quick_event(screens::QrMode::Circuit);
        self.game.quick.car = Some("PEUGOT".into());
        self.game.start_race(e, true, false).unwrap();
        self.ticks(300);
        self.game.key(KeyCode::Escape, &mut || {}).unwrap();
        self.wait(secs);
        self.act(Action::MainMenu, MenuKind::RacePause);
        self.into_menu(secs);
        self.resume(secs);
    }

    /// Yes through a quit question, then Back until the main menu.
    fn into_menu(&mut self, secs: f32) {
        for _ in 0..16 {
            if matches!(self.game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Main))) {
                self.wait(secs);
                return;
            }
            match &self.game.mode {
                Mode::Menu(Menu::Dialog(..)) => self.confirm(),
                _ => self.key(Key::Back),
            }
            self.wait(secs);
        }
        panic!("not on the main menu ({})", self.menu());
    }

    /// Free roam's encounter racers: car type, rating, class.
    fn racers(&self) -> Vec<(String, u32, u32)> {
        let e = self.game.sim.encounters.as_ref().expect("encounter racers");
        e.cars
            .iter()
            .map(|c| {
                let h = &c.car.car_type.handling;
                (c.car.car_type.name.clone(), h.performance_rating.to_bits(), h.performance_class as u32)
            })
            .collect()
    }
}

/// A free-roam entry as one of the two games made it: the cars taken
/// ahead, the generator after the set-up, the racers, the state over the
/// first second: the simulation's components and the v2 columns of the
/// clocks, the encounters, the HUD and the front end. Left out: the v2
/// globals' column, which holds the particles' generator (render side, read
/// by no simulation step): after a race's end it differs between two runs
/// of this walk with nothing loaded ahead in either.
#[derive(Debug, PartialEq)]
struct Entry {
    taken: (usize, usize),
    rng: u32,
    racers: Vec<(String, u32, u32)>,
    states: Vec<([u64; 5], [u64; 4])>,
}

impl Walk {
    fn entry(&mut self) -> Entry {
        let (taken, rng, racers) = (self.game.loads.encounters_taken, self.game.rng.0, self.racers());
        let states = (0..60)
            .map(|_| {
                self.ticks(1);
                let v2 = crate::dev::state_hash::components_v2(&self.game);
                (crate::dev::state_hash::components(&self.game), [v2[1], v2[2], v2[3], v2[4]])
            })
            .collect();
        Entry { taken, rng, racers, states }
    }
}

/// The ways into free roam walked in turn from the main menu.
const PATHS: [&str; 5] =
    ["Resume, the garage in a hurry", "a race's end", "a shop's exit", "Jump to Garage", "a Quick Race, then Resume"];

/// One game's walk (on a thread of its own: its own data folder and
/// session), each entry as it made it.
fn walk(stage: u8, wins: usize, ahead: bool) -> Vec<Entry> {
    let mut w = Walk::career(stage, wins, ahead);
    w.settle();
    PATHS
        .iter()
        .map(|what| {
            match *what {
                "Resume, the garage in a hurry" => w.resume(0.3),
                "a race's end" => w.race_and_back(1.0),
                "a shop's exit" => w.shop_and_back(1.0),
                "Jump to Garage" => w.jump_to_garage(1.0),
                _ => w.quick_race_then_resume(1.0),
            }
            w.entry()
        })
        .collect()
}

/// From a fresh career and from careers in their stages saved by the
/// harness, each way into career free roam — Resume Career and the
/// garage, a race's end and its post-race menu, a shop's exit, Jump to
/// Garage, a Quick Race left for the career — takes all its encounter
/// racers' cars loaded ahead (each screen up a second, as a player reads
/// it; the garage also as briefly as a player in a hurry has it, a third
/// of a second, and its question as long), and free roam (its racers, the
/// generator, the state of its first second) is the one set up with
/// nothing loaded ahead.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn every_free_roam_entry_takes_its_encounter_cars_loaded_ahead() {
    testkit::game();
    for (stage, wins) in [(1u8, 0usize), (2, 4), (4, 6)] {
        let a = std::thread::spawn(move || walk(stage, wins, true));
        let b = std::thread::spawn(move || walk(stage, wins, false));
        let (a, b) = (a.join().unwrap(), b.join().unwrap());
        for ((what, a), b) in PATHS.iter().zip(&a).zip(&b) {
            let (taken, asked) = a.taken;
            eprintln!("stage {stage}, {what}: {taken} of {asked} encounter cars taken from the loads ahead");
            assert!(asked > 0, "stage {stage}, {what}: encounter racers set up");
            assert_eq!(taken, asked, "stage {stage}, {what}: every encounter car taken from the loads ahead");
            assert_eq!(b.taken, (0, asked), "stage {stage}, {what}: nothing ahead in the other");
            assert_eq!((a.rng, &a.racers), (b.rng, &b.racers), "stage {stage}, {what}: the generator and the racers");
            for (t, (x, y)) in a.states.iter().zip(&b.states).enumerate() {
                assert_eq!(x, y, "stage {stage}, {what}: the state {t} ticks in");
            }
        }
    }
}
